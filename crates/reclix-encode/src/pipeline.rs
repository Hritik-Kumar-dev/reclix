//! FFmpeg encoding pipeline.

use std::{
    io::Write,
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    time::Duration,
};

use anyhow::{bail, Context};
use tracing::{debug, error, info, warn};

use crate::vaapi::probe_vaapi;

// ─── Config ───────────────────────────────────────────────────────────────────

/// Encoding configuration for a session.
#[derive(Debug, Clone)]
pub struct EncoderConfig {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    /// Input pixel format coming from PipeWire (e.g. "bgra", "nv12").
    pub pix_fmt: String,
    /// Audio sample rate in Hz.
    pub audio_sample_rate: u32,
    /// Audio channel count.
    pub audio_channels: u32,
    /// Audio sample format (e.g. "f32le", "s16le").
    pub audio_fmt: String,
    /// Target bitrate string for VA-API (e.g. "4M"). Ignored for x264.
    pub vaapi_bitrate: String,
    /// CRF value for x264 fallback.
    pub x264_crf: u32,
    /// Whether microphone audio is enabled.
    pub mic_enabled: bool,
}

impl Default for EncoderConfig {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            fps: 30,
            pix_fmt: "bgra".into(),
            audio_sample_rate: 48000,
            audio_channels: 2,
            audio_fmt: "f32le".into(),
            vaapi_bitrate: "4M".into(),
            x264_crf: 23,
            mic_enabled: true,
        }
    }
}

// ─── Encoder backend ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoBackend {
    VaApi,
    X264,
}

impl std::fmt::Display for VideoBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VideoBackend::VaApi => write!(f, "h264_vaapi"),
            VideoBackend::X264  => write!(f, "libx264"),
        }
    }
}

// ─── Pipeline ────────────────────────────────────────────────────────────────

/// Top-level encoding pipeline for a session.
/// Owns the encoder backend choice and spawns per-segment encoders.
pub struct EncoderPipeline {
    pub backend: VideoBackend,
    config: EncoderConfig,
}

impl EncoderPipeline {
    /// Create a new pipeline, probing VA-API availability.
    pub fn new(config: EncoderConfig) -> Self {
        let backend = if probe_vaapi() {
            VideoBackend::VaApi
        } else {
            warn!("VA-API unavailable, falling back to libx264");
            VideoBackend::X264
        };
        info!("encoder backend: {backend}");
        Self { backend, config }
    }

    /// Create a pipeline with an explicit backend (useful in tests).
    pub fn with_backend(config: EncoderConfig, backend: VideoBackend) -> Self {
        Self { backend, config }
    }

    /// Open a new segment encoder writing to `output_path` (a `.mkv` file).
    pub fn open_segment(
        &self,
        output_path: &Path,
    ) -> anyhow::Result<SegmentEncoder> {
        SegmentEncoder::spawn(&self.config, self.backend, output_path)
    }

    /// Concatenate all segment files listed in `segment_paths` into a final
    /// MP4 at `output_path`.
    ///
    /// Uses the FFmpeg concat demuxer — no re-encoding.
    pub fn concat_segments(
        segment_paths: &[PathBuf],
        output_path: &Path,
    ) -> anyhow::Result<()> {
        if segment_paths.is_empty() {
            bail!("no segments to concatenate");
        }

        // Write a temporary concat list file next to the output.
        let concat_file = output_path.with_extension("concat.txt");
        {
            let mut f = std::fs::File::create(&concat_file)
                .context("could not create concat list file")?;
            for seg in segment_paths {
                // Paths must be absolute or relative to the concat file's dir.
                writeln!(f, "file '{}'", seg.display())
                    .context("writing concat list")?;
            }
        }

        info!(
            segments = segment_paths.len(),
            output = %output_path.display(),
            "concatenating segments"
        );

        let status = Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel", "warning",
                "-y",
                "-f", "concat",
                "-safe", "0",
                "-i", &concat_file.to_string_lossy(),
                "-c", "copy",
                "-movflags", "+faststart",
                &output_path.to_string_lossy(),
            ])
            .status()
            .context("ffmpeg concat: could not spawn ffmpeg")?;

        // Clean up temp file regardless.
        let _ = std::fs::remove_file(&concat_file);

        if !status.success() {
            bail!("ffmpeg concat exited with status {status}");
        }

        info!(output = %output_path.display(), "export complete");
        Ok(())
    }
}

// ─── Per-segment encoder ──────────────────────────────────────────────────────

/// An active FFmpeg process encoding one recording segment.
///
/// Video and audio are fed via separate named pipes (FIFOs) to avoid
/// multiplexing them in Rust — FFmpeg handles the A/V sync.
pub struct SegmentEncoder {
    process: Child,
    /// Write raw video frames here.
    pub video_stdin: ChildStdin,
    /// Write raw audio samples here (None if mic disabled).
    pub audio_stdin: Option<ChildStdin>,
    output_path: PathBuf,
}

impl SegmentEncoder {
    fn spawn(
        config: &EncoderConfig,
        backend: VideoBackend,
        output_path: &Path,
    ) -> anyhow::Result<Self> {
        // ── Build the FFmpeg command ──────────────────────────────────────────
        //
        // We pipe raw video on stdin (fd 0) and raw audio on a second pipe
        // opened as fd 3 via process_raw_fd / pipe().  For simplicity in V0.1
        // we use two separate ffmpeg invocations: one for video-only and one
        // mux pass, OR we use the `-thread_queue_size` trick with two `-i pipe:`
        // inputs.  The cleanest cross-platform approach: use two stdin pipes
        // via separate sub-processes for video and audio and let ffmpeg mux
        // from named pipes.
        //
        // Actually the simplest robust approach on Linux: open two anonymous
        // pipes, pass them as /dev/fd/N.

        let (video_read_fd, video_write_fd) = nix_pipe()?;
        let (audio_read_fd, audio_write_fd) = if config.mic_enabled {
            let (r, w) = nix_pipe()?;
            (Some(r), Some(w))
        } else {
            (None, None)
        };

        let video_input_arg = format!("/dev/fd/{video_read_fd}");

        // ── Video filter chain ────────────────────────────────────────────────
        let vfilter = match backend {
            VideoBackend::VaApi => {
                format!("format=nv12,hwupload")
            }
            VideoBackend::X264 => {
                // x264 wants yuv420p.
                format!("format=yuv420p")
            }
        };

        let mut cmd = Command::new("ffmpeg");
        cmd.args(["-hide_banner", "-loglevel", "warning", "-y"]);

        // ── Video input ───────────────────────────────────────────────────────
        cmd.args([
            "-f", "rawvideo",
            "-pix_fmt", &config.pix_fmt,
            "-s", &format!("{}x{}", config.width, config.height),
            "-r", &config.fps.to_string(),
            "-i", &video_input_arg,
        ]);

        // ── Audio input ───────────────────────────────────────────────────────
        if config.mic_enabled {
            if let Some(ar_fd) = audio_read_fd {
                let audio_input_arg = format!("/dev/fd/{ar_fd}");
                cmd.args([
                    "-f", &config.audio_fmt,
                    "-ar", &config.audio_sample_rate.to_string(),
                    "-ac", &config.audio_channels.to_string(),
                    "-i", &audio_input_arg,
                ]);
            }
        }

        // ── Video codec ───────────────────────────────────────────────────────
        cmd.args(["-vf", &vfilter]);
        match backend {
            VideoBackend::VaApi => {
                cmd.args([
                    "-c:v", "h264_vaapi",
                    "-b:v", &config.vaapi_bitrate,
                ]);
            }
            VideoBackend::X264 => {
                cmd.args([
                    "-c:v", "libx264",
                    "-preset", "veryfast",
                    "-crf", &config.x264_crf.to_string(),
                    "-tune", "zerolatency",
                ]);
            }
        }

        // ── Audio codec ───────────────────────────────────────────────────────
        if config.mic_enabled {
            cmd.args(["-c:a", "aac", "-b:a", "128k"]);
        }

        // ── Output ────────────────────────────────────────────────────────────
        cmd.args([
            "-f", "matroska",   // MKV container for segments
            &output_path.to_string_lossy().to_string(),
        ]);

        // Keep the write ends of pipes open in the child by not closing them.
        // SAFETY: we use pre-opened file descriptors.
        unsafe {
            use std::os::unix::process::CommandExt;
            let vfd = video_read_fd;
            let afd = audio_read_fd;
            cmd.pre_exec(move || {
                // The read-ends are already inherited; close the write ends
                // inside the child so they don't block EOF detection.
                // (Nothing to do here — the write ends are in separate Stdio
                //  objects on the parent side.)
                Ok(())
            });
        }

        // Inherit stdin/stdout so ffmpeg can receive /dev/fd/N.
        cmd.stdin(Stdio::null())
           .stdout(Stdio::null())
           .stderr(Stdio::inherit());

        let process = cmd.spawn().context("could not spawn ffmpeg for segment encoding")?;

        // Wrap the write ends in File so we can impl Write on them.
        use std::os::unix::io::FromRawFd;
        let video_stdin_file = unsafe { std::fs::File::from_raw_fd(video_write_fd) };
        let audio_stdin_file = audio_write_fd.map(|fd| unsafe { std::fs::File::from_raw_fd(fd) });

        // Close the read ends in the parent now that the child has them.
        unsafe {
            libc::close(video_read_fd);
            if let Some(rfd) = audio_read_fd {
                libc::close(rfd);
            }
        }

        info!(output = %output_path.display(), backend = %backend, "segment encoder started");

        Ok(Self {
            process,
            video_stdin: video_stdin_file.into(),
            audio_stdin: audio_stdin_file.map(Into::into),
            output_path: output_path.to_owned(),
        })
    }

    /// Write a raw video frame to the encoder.
    pub fn write_video_frame(&mut self, data: &[u8]) -> anyhow::Result<()> {
        self.video_stdin
            .write_all(data)
            .context("write_video_frame: pipe write failed")
    }

    /// Write raw audio samples to the encoder.
    pub fn write_audio_samples(&mut self, data: &[u8]) -> anyhow::Result<()> {
        if let Some(ref mut stdin) = self.audio_stdin {
            stdin.write_all(data).context("write_audio_samples: pipe write failed")?;
        }
        Ok(())
    }

    /// Finish encoding: close pipes and wait for ffmpeg to exit.
    /// Returns the output path on success.
    pub fn finish(mut self) -> anyhow::Result<PathBuf> {
        // Drop stdin handles to signal EOF to ffmpeg.
        drop(self.video_stdin);
        drop(self.audio_stdin.take());

        let status = self
            .process
            .wait()
            .context("waiting for ffmpeg segment encoder")?;

        if !status.success() {
            bail!(
                "ffmpeg segment encoder exited with status {status} for {}",
                self.output_path.display()
            );
        }

        info!(output = %self.output_path.display(), "segment encoded");
        Ok(self.output_path)
    }
}

// ─── helpers ─────────────────────────────────────────────────────────────────

/// Create an anonymous OS pipe and return (read_fd, write_fd).
fn nix_pipe() -> anyhow::Result<(i32, i32)> {
    let mut fds = [0i32; 2];
    let ret = unsafe { libc::pipe(fds.as_mut_ptr()) };
    if ret != 0 {
        bail!("pipe(2) failed: {}", std::io::Error::last_os_error());
    }
    Ok((fds[0], fds[1]))
}

// ─── tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_sanity() {
        let cfg = EncoderConfig::default();
        assert_eq!(cfg.fps, 30);
        assert_eq!(cfg.width, 1920);
        assert_eq!(cfg.x264_crf, 23);
    }

    #[test]
    fn concat_empty_segments_errors() {
        let out = PathBuf::from("/tmp/reclix_test_out.mp4");
        let result = EncoderPipeline::concat_segments(&[], &out);
        assert!(result.is_err());
    }
}
