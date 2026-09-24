//! PipeWire microphone capture loop.

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Instant,
};

use anyhow::Context;
use pipewire::{
    self as pw,
    prelude::ReadableDict,
    spa::{
        param::audio::{AudioFormat, AudioInfoRaw},
        pod::Pod,
        utils::Direction,
    },
    stream::{Stream, StreamFlags, StreamState},
};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::audio_frame::{AudioFrame, SampleFormat};

/// Handle to the running microphone capture thread.
pub struct MicHandle {
    /// When `false`, captured samples are silently discarded (muted).
    pub active: Arc<AtomicBool>,
    quit_tx: std::sync::mpsc::SyncSender<()>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl MicHandle {
    pub fn mute(&self) {
        self.active.store(false, Ordering::Release);
        info!("microphone muted");
    }

    pub fn unmute(&self) {
        self.active.store(true, Ordering::Release);
        info!("microphone unmuted");
    }

    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }

    pub fn stop(mut self) {
        let _ = self.quit_tx.send(());
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for MicHandle {
    fn drop(&mut self) {
        let _ = self.quit_tx.send(());
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Start capturing from the default PipeWire microphone source.
///
/// * `sample_rate` — desired sample rate (e.g. 48000)
/// * `channels`    — desired channel count (1 = mono, 2 = stereo)
/// * `frame_tx`    — channel where [`AudioFrame`]s are sent to the encoder
pub fn start_mic_capture(
    sample_rate: u32,
    channels: u32,
    frame_tx: mpsc::Sender<AudioFrame>,
) -> anyhow::Result<MicHandle> {
    let active = Arc::new(AtomicBool::new(true));
    let active_clone = Arc::clone(&active);

    let (quit_tx, quit_rx) = std::sync::mpsc::sync_channel::<()>(1);

    let thread = std::thread::Builder::new()
        .name("reclix-mic".into())
        .spawn(move || {
            if let Err(e) =
                mic_thread(sample_rate, channels, frame_tx, active_clone, quit_rx)
            {
                error!("microphone capture thread error: {e:#}");
            }
        })
        .context("failed to spawn microphone thread")?;

    info!(sample_rate, channels, "microphone capture thread started");
    Ok(MicHandle { active, quit_tx, thread: Some(thread) })
}

// ─── microphone thread ────────────────────────────────────────────────────────

fn mic_thread(
    sample_rate: u32,
    channels: u32,
    frame_tx: mpsc::Sender<AudioFrame>,
    active: Arc<AtomicBool>,
    quit_rx: std::sync::mpsc::Receiver<()>,
) -> anyhow::Result<()> {
    pw::init();

    let main_loop = pw::main_loop::MainLoop::new(None)
        .context("PipeWire audio: could not create MainLoop")?;

    let context = pw::context::Context::new(&main_loop)
        .context("PipeWire audio: could not create Context")?;

    let core = context
        .connect(None)
        .context("PipeWire audio: could not connect to default server")?;

    let epoch = Instant::now();
    let frame_tx = frame_tx.clone();
    let active_cb = Arc::clone(&active);

    let mut audio_info: Option<AudioInfoRaw> = None;

    // Build audio capture stream targeting the default microphone.
    let stream = Stream::new(&core, "reclix-mic", pw::properties! {
        *pw::keys::MEDIA_TYPE     => "Audio",
        *pw::keys::MEDIA_CATEGORY => "Capture",
        *pw::keys::MEDIA_ROLE     => "Communication",
        // Target the default microphone (empty string = let PipeWire choose).
        *pw::keys::NODE_TARGET    => "",
    })
    .context("PipeWire audio: could not create Stream")?;

    let _listener = stream
        .add_local_listener_with_user_data(())
        .state_changed(|_, _, old, new| {
            debug!("mic stream state: {old:?} → {new:?}");
            if matches!(new, StreamState::Error(..)) {
                error!("mic stream error state: {new:?}");
            }
        })
        .param_changed(move |_, _, id, pod| {
            use pipewire::spa::param::ParamType;
            if id != ParamType::Format.as_raw() { return; }
            if let Some(pod) = pod {
                let mut info = AudioInfoRaw::default();
                if info.parse(pod).is_ok() {
                    debug!(
                        rate   = info.rate(),
                        chans  = info.channels(),
                        "mic audio format negotiated"
                    );
                    audio_info = Some(info);
                }
            }
        })
        .process(move |stream, _| {
            if !active_cb.load(Ordering::Acquire) {
                if let Some(mut buf) = stream.dequeue_buffer() { drop(buf); }
                return;
            }

            let Some(mut buf) = stream.dequeue_buffer() else { return };
            let datas = buf.datas_mut();
            if datas.is_empty() { return; }

            let d = &datas[0];
            let chunk = match d.chunk() { Some(c) => c, None => return };
            let size = chunk.size() as usize;
            if size == 0 { return; }
            let slice = match d.data() { Some(s) => s, None => return };

            let (sr, ch, fmt) = audio_info
                .as_ref()
                .map(|ai| {
                    let f = match ai.format() {
                        AudioFormat::S16LE => SampleFormat::S16Le,
                        _                  => SampleFormat::F32Le,
                    };
                    (ai.rate(), ai.channels(), f)
                })
                .unwrap_or((sample_rate, channels, SampleFormat::F32Le));

            let frame = AudioFrame {
                pts:         epoch.elapsed(),
                format:      fmt,
                channels:    ch,
                sample_rate: sr,
                data:        slice[..size].to_vec(),
            };

            let _ = frame_tx.try_send(frame);
        })
        .register()
        .context("PipeWire audio: listener registration failed")?;

    // Build a format pod for the desired sample rate / channels.
    // Using RAW audio format negotiation.
    stream
        .connect(
            Direction::Input,
            None,
            StreamFlags::AUTOCONNECT | StreamFlags::MAP_BUFFERS | StreamFlags::RT_PROCESS,
            &mut [],
        )
        .context("PipeWire audio: stream connect failed")?;

    info!("mic PipeWire stream connected, entering main loop");

    let ml = main_loop.clone();
    let _quit_source = main_loop.loop_().add_timer(move |_| {
        if quit_rx.try_recv().is_ok() {
            ml.quit();
        }
    });

    main_loop.run();

    info!("mic PipeWire main loop exited");
    pw::deinit();
    Ok(())
}
