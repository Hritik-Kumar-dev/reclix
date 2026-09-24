//! VA-API availability probe.
//!
//! Runs `ffmpeg -hide_banner -f lavfi -i color=c=black:s=16x16:r=1 -t 0.1
//!        -vf format=nv12,hwupload -c:v h264_vaapi -f null -`
//! and checks the exit code.  Fast (< 300 ms) and does not produce output.

use std::process::Command;

use tracing::{debug, info, warn};

/// Returns `true` if `h264_vaapi` is usable on this system.
pub fn probe_vaapi() -> bool {
    debug!("probing VA-API availability");
    let result = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel", "error",
            "-f", "lavfi",
            "-i", "color=c=black:s=16x16:r=1:d=0.1",
            "-vf", "format=nv12,hwupload",
            "-c:v", "h264_vaapi",
            "-f", "null",
            "-",
        ])
        .output();

    match result {
        Ok(out) if out.status.success() => {
            info!("VA-API h264_vaapi encoder available");
            true
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            debug!("VA-API probe failed ({}): {}", out.status, stderr.trim());
            false
        }
        Err(e) => {
            warn!("ffmpeg not found or could not be executed: {e}");
            false
        }
    }
}
