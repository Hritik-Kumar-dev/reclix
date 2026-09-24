//! Desktop notifications via `notify-send` (libnotify CLI).
//!
//! We shell out to `notify-send` rather than linking libnotify to keep the
//! dependency surface minimal.  Failures are logged but never fatal.

use std::{path::Path, process::Command};
use tracing::warn;

/// Show a "Recording complete" notification with the output file path.
pub fn recording_complete(output_file: &Path) {
    let body = format!("Saved to {}", output_file.display());
    let result = Command::new("notify-send")
        .args([
            "--app-name=Reclix",
            "--icon=media-record",
            "--urgency=normal",
            "Recording complete",
            &body,
        ])
        .status();

    if let Err(e) = result {
        warn!("notify-send failed: {e}");
    }
}

/// Show an error notification.
pub fn show_error(summary: &str, body: &str) {
    let result = Command::new("notify-send")
        .args([
            "--app-name=Reclix",
            "--icon=dialog-error",
            "--urgency=critical",
            summary,
            body,
        ])
        .status();

    if let Err(e) = result {
        warn!("notify-send (error) failed: {e}");
    }
}
