//! Orchestrator — owns the active session, capture handles, encoder, and tray.

use std::{
    path::PathBuf,
    sync::{Arc, Mutex as StdMutex},
};

use anyhow::{bail, Context};
use tracing::{info, warn};

use reclix_core::{
    config::ReclixConfig,
    dbus_types::ServiceStatus,
    session::{ensure_output_dir, session_dir_name, AudioConfig, Session, VideoSource},
    state::{RecordingState, StateEvent},
};
use reclix_audio::MicHandle;
use reclix_capture::CaptureHandle;
use reclix_encode::{EncoderConfig, EncoderPipeline, SegmentEncoder};
use reclix_tray::TrayState;

pub struct ActiveSession {
    pub session:  Session,
    pub capture:  CaptureHandle,
    pub mic:      Option<MicHandle>,
    pub encoder:  Option<SegmentEncoder>,
    pub pipeline: EncoderPipeline,
}

pub struct Orchestrator {
    pub config:     ReclixConfig,
    pub active:     Option<ActiveSession>,
    pub tray_state: Arc<StdMutex<TrayState>>,
}

impl Orchestrator {
    pub fn new(config: ReclixConfig, tray_state: Arc<StdMutex<TrayState>>) -> Self {
        Self { config, active: None, tray_state }
    }

    pub fn status(&self) -> ServiceStatus {
        match &self.active {
            None => ServiceStatus {
                state:          RecordingState::Idle,
                recording_secs: 0,
                segment_index:  None,
                mic_enabled:    self.config.mic_enabled_by_default,
                camera_enabled: false,
                output_file:    None,
            },
            Some(a) => ServiceStatus {
                state:          a.session.state(),
                recording_secs: a.session.total_recording_secs(),
                segment_index:  a.session.segments.last().map(|s| s.index),
                mic_enabled:    a.mic.as_ref().map(|m| m.is_active()).unwrap_or(false),
                camera_enabled: false,
                output_file:    a.session.output_file.as_ref()
                                  .map(|p| p.to_string_lossy().to_string()),
            },
        }
    }

    pub fn sync_tray(&self) {
        let s = self.status();
        if let Ok(mut ts) = self.tray_state.lock() {
            ts.recording_state   = s.state;
            ts.elapsed_secs      = s.recording_secs;
            ts.mic_enabled       = s.mic_enabled;
            ts.sys_audio_enabled = false;
        }
    }

    // ── Command handlers ───────────────────────────────────────────────────

    pub async fn handle_start(&mut self) -> anyhow::Result<()> {
        if self.active.is_some() {
            bail!("a session is already active");
        }
        info!("starting new recording session");

        // 1. Session directory
        let output_dir = ensure_output_dir()
            .context("could not create ~/Videos/Reclix/")?;
        let session_dir = output_dir.join(session_dir_name());
        std::fs::create_dir_all(&session_dir)
            .context("could not create session directory")?;

        // 2. Session object
        let audio_cfg = AudioConfig {
            mic_enabled:          self.config.mic_enabled_by_default,
            system_audio_enabled: false,
        };
        let mut session = Session::new(
            session_dir.clone(),
            VideoSource::FullScreen,
            audio_cfg,
        );
        session.apply(StateEvent::Start)?;

        // 3. Portal + capture — only available with pipewire-capture feature
        #[cfg(feature = "full")]
        let (capture, width, height) = {
            let source = reclix_capture::portal::request_screencast()
                .await
                .map_err(|e| reclix_core::error::ReclixError::PortalDenied(e.to_string()))?;
            let w = if source.width  > 0 { source.width  } else { 1920 };
            let h = if source.height > 0 { source.height } else { 1080 };
            let (frame_tx, _frame_rx) = tokio::sync::mpsc::channel(128);
            let cap = reclix_capture::start_capture(source.fd, source.node_id, self.config.fps, frame_tx)
                .map_err(|e| reclix_core::error::ReclixError::PipeWire(e.to_string()))?;
            (cap, w, h)
        };

        #[cfg(not(feature = "full"))]
        #[allow(clippy::diverging_sub_expression)]
        let (capture, width, height): (CaptureHandle, u32, u32) = {
            bail!("reclix-service built without 'full' feature — PipeWire capture unavailable.\nRebuild with: cargo build --features full");
        };

        // 4. Microphone
        #[cfg(feature = "full")]
        let mic = if self.config.mic_enabled_by_default {
            let (audio_tx, _audio_rx) = tokio::sync::mpsc::channel(256);
            match reclix_audio::start_mic_capture(48_000, 2, audio_tx) {
                Ok(h)  => { info!("microphone capture started"); Some(h) }
                Err(e) => { warn!("microphone unavailable: {e}"); None }
            }
        } else {
            None
        };

        #[cfg(not(feature = "full"))]
        #[allow(unreachable_code)]
        let mic: Option<MicHandle> = None;

        // 5. Encoder pipeline + first segment
        let enc_cfg = EncoderConfig {
            width,
            height,
            fps:               self.config.fps,
            pix_fmt:           "bgra".into(),
            audio_sample_rate: 48_000,
            audio_channels:    2,
            audio_fmt:         "f32le".into(),
            vaapi_bitrate:     self.config.vaapi_bitrate.clone(),
            x264_crf:          self.config.x264_crf,
            mic_enabled:       mic.is_some(),
        };
        let pipeline = EncoderPipeline::new(enc_cfg);

        // Open first segment — borrow ends before we store session.
        let seg_video_file = {
            let seg = session.open_segment();
            seg.video_file.clone()
        };
        let seg_path = session_dir.join(&seg_video_file);
        let encoder = pipeline.open_segment(&seg_path)
            .context("could not open segment encoder")?;

        // 6. Advance to RECORDING
        session.apply(StateEvent::Ready)?;
        session.save().ok();

        self.active = Some(ActiveSession {
            session,
            capture,
            mic,
            encoder: Some(encoder),
            pipeline,
        });
        self.sync_tray();
        info!("recording started ({}×{})", width, height);
        Ok(())
    }

    pub async fn handle_pause_resume(&mut self) -> anyhow::Result<()> {
        let a = self.active.as_mut().context("no active session")?;
        match a.session.state() {
            RecordingState::Recording => {
                a.session.apply(StateEvent::Pause)?;
                a.session.close_current_segment();
                if let Some(enc) = a.encoder.take() {
                    enc.finish().context("closing encoder on pause")?;
                }
                a.capture.pause();
                a.session.save().ok();
                // Sync tray — must reborrow self after mutable borrow of a ends.
            }
            RecordingState::Paused => {
                a.session.apply(StateEvent::Resume)?;
                // Clone the path we need before the mutable borrow.
                let (seg_video_file, seg_index) = {
                    let seg = a.session.open_segment();
                    (seg.video_file.clone(), seg.index)
                };
                let seg_path = a.session.session_dir.join(&seg_video_file);
                let encoder = a.pipeline
                    .open_segment(&seg_path)
                    .context("could not open encoder on resume")?;
                a.encoder = Some(encoder);
                a.capture.resume();
                a.session.save().ok();
                info!("recording resumed (segment {})", seg_index);
            }
            s => bail!("pause_resume called in state {s}"),
        }
        self.sync_tray();
        Ok(())
    }

    pub async fn handle_finish(&mut self) -> anyhow::Result<()> {
        let mut a = self.active.take().context("no active session")?;

        a.session.close_current_segment();
        a.session.apply(StateEvent::Finish)?;

        if let Some(enc) = a.encoder.take() {
            enc.finish().context("closing encoder on finish")?;
        }

        // Collect everything we need before destructuring `a`.
        let seg_paths: Vec<PathBuf> = a
            .session
            .exportable_segments()
            .iter()
            .map(|s| a.session.session_dir.join(&s.video_file))
            .collect();

        let output_file = {
            let parent = a.session.session_dir.parent()
                .unwrap_or(&a.session.session_dir)
                .to_path_buf();
            let stem = a.session.session_dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("reclix_recording");
            parent.join(format!("{stem}.mp4"))
        };

        // Destructure to cleanly drop capture+mic without partial-move issues.
        let ActiveSession { mut session, capture, mic, encoder: _, pipeline: _ } = a;
        capture.pause();
        // CaptureHandle/MicHandle are stub types when `full` is disabled — no Drop impl.
        #[allow(clippy::drop_non_drop)]
        {
            drop(capture);
            drop(mic);
        }

        info!(output = %output_file.display(), "concatenating {} segment(s)", seg_paths.len());

        let seg_paths_c   = seg_paths.clone();
        let output_file_c = output_file.clone();
        tokio::task::spawn_blocking(move || {
            EncoderPipeline::concat_segments(&seg_paths_c, &output_file_c)
        })
        .await
        .context("spawn_blocking panicked")?
        .context("concat_segments failed")?;

        session.output_file = Some(output_file.clone());
        session.apply(StateEvent::Done)?;
        session.save().ok();

        // Update tray directly — we can't put session back without a full
        // ActiveSession, so write tray fields manually.
        if let Ok(mut ts) = self.tray_state.lock() {
            ts.recording_state = RecordingState::Completed;
            ts.elapsed_secs    = session.total_recording_secs();
        }

        info!(output = %output_file.display(), "export complete ✓");
        crate::notify::recording_complete(&output_file);

        // Return to IDLE.
        if let Ok(mut ts) = self.tray_state.lock() {
            ts.recording_state = RecordingState::Idle;
            ts.elapsed_secs    = 0;
        }
        Ok(())
    }

    pub async fn handle_toggle_mic(&mut self) -> anyhow::Result<()> {
        match &self.active {
            Some(a) => match &a.mic {
                Some(mic) => {
                    if mic.is_active() { mic.mute(); } else { mic.unmute(); }
                    self.sync_tray();
                }
                None => warn!("toggle_mic: no microphone capture active"),
            },
            None => bail!("no active session"),
        }
        Ok(())
    }
}
