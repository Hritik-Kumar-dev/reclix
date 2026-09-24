//! PipeWire capture loop.
//!
//! Runs the PipeWire `MainLoop` on a dedicated OS thread (PipeWire is not
//! async-runtime-aware).  Frames are forwarded to the encoder via a
//! `tokio::sync::mpsc` channel.
//!
//! # Pause / resume
//! We do NOT pause PipeWire itself — that would require renegotiating the
//! portal session.  Instead the service sets `active = false` on the handle
//! and the capture loop discards frames silently.  This keeps latency low on
//! resume and avoids portal round-trips.

use std::{
    os::fd::{FromRawFd, IntoRawFd, OwnedFd},
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
        self,
        param::video::{VideoFormat, VideoInfoRaw},
        pod::Pod,
        utils::Direction,
    },
    stream::{Stream, StreamFlags, StreamState},
};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::frame::{FrameFormat, RawFrame};

/// A handle to the running PipeWire capture thread.
/// Drop this to stop the capture loop.
pub struct CaptureHandle {
    /// Set to `false` to make the loop discard frames (pause) without stopping.
    pub active: Arc<AtomicBool>,
    /// Signal the PipeWire main loop to quit.
    quit_tx: std::sync::mpsc::SyncSender<()>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl CaptureHandle {
    pub fn pause(&self) {
        self.active.store(false, Ordering::Release);
    }

    pub fn resume(&self) {
        self.active.store(true, Ordering::Release);
    }

    pub fn stop(mut self) {
        let _ = self.quit_tx.send(());
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for CaptureHandle {
    fn drop(&mut self) {
        let _ = self.quit_tx.send(());
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Start capturing from a PipeWire node obtained via the XDG portal.
///
/// * `fd`      — the PipeWire remote fd from `portal::request_screencast`
/// * `node_id` — the stream node id from the portal
/// * `fps`     — target frame rate (informational; the portal controls actual rate)
/// * `frame_tx`— channel where captured [`RawFrame`]s are sent
///
/// Returns a [`CaptureHandle`] that controls the running thread.
pub fn start_capture(
    fd: OwnedFd,
    node_id: u32,
    fps: u32,
    frame_tx: mpsc::Sender<RawFrame>,
) -> anyhow::Result<CaptureHandle> {
    let active = Arc::new(AtomicBool::new(true));
    let active_clone = Arc::clone(&active);

    let (quit_tx, quit_rx) = std::sync::mpsc::sync_channel::<()>(1);

    let thread = std::thread::Builder::new()
        .name("reclix-pipewire".into())
        .spawn(move || {
            if let Err(e) = capture_thread(fd, node_id, fps, frame_tx, active_clone, quit_rx) {
                error!("PipeWire capture thread error: {e:#}");
            }
        })
        .context("failed to spawn PipeWire thread")?;

    info!(node_id, "PipeWire capture thread started");
    Ok(CaptureHandle {
        active,
        quit_tx,
        thread: Some(thread),
    })
}

// ─── PipeWire thread ──────────────────────────────────────────────────────────

fn capture_thread(
    fd: OwnedFd,
    node_id: u32,
    fps: u32,
    frame_tx: mpsc::Sender<RawFrame>,
    active: Arc<AtomicBool>,
    quit_rx: std::sync::mpsc::Receiver<()>,
) -> anyhow::Result<()> {
    pw::init();

    // Create a PipeWire main loop.
    let main_loop = pw::main_loop::MainLoop::new(None)
        .context("PipeWire: could not create MainLoop")?;

    // Connect using the portal-provided fd.
    let raw_fd = fd.into_raw_fd();
    // SAFETY: raw_fd is valid and we take ownership.
    let context = pw::context::Context::new(&main_loop)
        .context("PipeWire: could not create Context")?;
    let core = unsafe {
        context
            .connect_fd(
                spa::utils::result::SpaResult::from_i32(raw_fd)
                    .ok()
                    .and_then(|_| None::<()>)
                    .map_or(raw_fd, |_| raw_fd),
                None,
            )
    }
    .context("PipeWire: connect_fd failed")?;

    // Epoch for PTS calculation.
    let epoch = Instant::now();

    // State shared into the process callback.
    let frame_tx = frame_tx.clone();
    let active_cb = Arc::clone(&active);
    let mut video_info: Option<VideoInfoRaw> = None;

    // Build a PipeWire stream.
    let stream = Stream::new(&core, "reclix-capture", pw::properties! {
        *pw::keys::MEDIA_TYPE       => "Video",
        *pw::keys::MEDIA_CATEGORY   => "Capture",
        *pw::keys::MEDIA_ROLE       => "Screen",
        *pw::keys::NODE_TARGET      => node_id.to_string(),
    })
    .context("PipeWire: could not create Stream")?;

    // ── Stream listeners ──────────────────────────────────────────────────────
    let _listener = stream
        .add_local_listener_with_user_data(())
        .state_changed(|_, _, old, new| {
            debug!("PipeWire stream state: {old:?} → {new:?}");
            if matches!(new, StreamState::Error(..)) {
                error!("PipeWire stream entered error state: {new:?}");
            }
        })
        .param_changed(move |_, _, id, pod| {
            if id != spa::param::ParamType::Format.as_raw() {
                return;
            }
            if let Some(pod) = pod {
                let mut info = VideoInfoRaw::default();
                if info.parse(pod).is_ok() {
                    debug!(
                        width = info.size().width,
                        height = info.size().height,
                        "PipeWire video format negotiated"
                    );
                    video_info = Some(info);
                }
            }
        })
        .process(move |stream, _| {
            if !active_cb.load(Ordering::Acquire) {
                // Paused: dequeue and discard buffer to keep PipeWire happy.
                if let Some(mut buf) = stream.dequeue_buffer() {
                    drop(buf);
                }
                return;
            }

            let Some(mut buf) = stream.dequeue_buffer() else { return };
            let datas = buf.datas_mut();
            if datas.is_empty() { return; }

            let d = &datas[0];
            let chunk = match d.chunk() {
                Some(c) => c,
                None => return,
            };
            let size = chunk.size() as usize;
            if size == 0 { return; }

            let slice = match d.data() {
                Some(s) => s,
                None => return,
            };

            // Resolve format from negotiated video_info; default to BGRA.
            let (width, height, stride, fmt) = video_info
                .as_ref()
                .map(|vi| {
                    let w = vi.size().width;
                    let h = vi.size().height;
                    let s = if w > 0 { (size as u32) / h.max(1) } else { w * 4 };
                    // Map SPA video format to our FrameFormat.
                    let f = match vi.format() {
                        VideoFormat::BGRA | VideoFormat::BGRx => FrameFormat::Bgra,
                        VideoFormat::RGBA | VideoFormat::RGBx => FrameFormat::Rgba,
                        VideoFormat::NV12 => FrameFormat::Nv12,
                        _ => FrameFormat::Bgra,
                    };
                    (w, h, s, f)
                })
                .unwrap_or((0, 0, 0, FrameFormat::Bgra));

            let frame = RawFrame {
                pts:    epoch.elapsed(),
                width,
                height,
                stride,
                format: fmt,
                data:   slice[..size].to_vec(),
            };

            // Non-blocking send: if the encoder is overwhelmed, drop the frame.
            if let Err(_e) = frame_tx.try_send(frame) {
                // Dropped frame — acceptable; encoder will recover on next.
            }
        })
        .register()
        .context("PipeWire: listener registration failed")?;

    // Connect the stream in passive consumer mode.
    let params: Vec<Box<dyn AsRef<Pod>>> = Vec::new(); // portal negotiates format
    stream
        .connect(
            Direction::Input,
            Some(node_id),
            StreamFlags::AUTOCONNECT | StreamFlags::MAP_BUFFERS,
            &mut [],
        )
        .context("PipeWire: stream connect failed")?;

    info!("PipeWire stream connected, entering main loop");

    // Run until signalled to quit.
    let ml = main_loop.clone();
    let _quit_source = main_loop
        .loop_()
        .add_timer(move |_| {
            // Poll quit channel without blocking.
            if quit_rx.try_recv().is_ok() {
                ml.quit();
            }
        });

    main_loop.run();

    info!("PipeWire main loop exited");
    pw::deinit();
    Ok(())
}
