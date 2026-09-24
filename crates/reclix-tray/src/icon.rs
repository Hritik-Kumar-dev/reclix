//! Tray icon name resolution.
//!
//! We prefer named XDG icon theme icons so the tray blends with the desktop
//! theme.  Fall back to the app icon name "reclix" if theme icons are absent.

use reclix_core::state::RecordingState;

/// Return the XDG icon name for the given recording state.
pub fn icon_name(state: RecordingState) -> &'static str {
    match state {
        RecordingState::Idle      => "media-record",
        RecordingState::Starting  => "content-loading-symbolic",
        RecordingState::Recording => "media-record-symbolic",   // red dot on most themes
        RecordingState::Paused    => "media-playback-pause-symbolic",
        RecordingState::Finishing => "content-loading-symbolic",
        RecordingState::Completed => "emblem-ok-symbolic",
    }
}

/// Build the tray title string shown as a tooltip and in the label.
pub fn tray_title(state: RecordingState, elapsed_secs: u64) -> String {
    match state {
        RecordingState::Idle      => "Reclix — idle".into(),
        RecordingState::Starting  => "Reclix — starting…".into(),
        RecordingState::Recording => format!("🔴 {}", fmt_duration(elapsed_secs)),
        RecordingState::Paused    => format!("⏸  {}", fmt_duration(elapsed_secs)),
        RecordingState::Finishing => "Reclix — exporting…".into(),
        RecordingState::Completed => "Reclix — done".into(),
    }
}

fn fmt_duration(secs: u64) -> String {
    let m = secs / 60;
    let s = secs % 60;
    format!("{m:02}:{s:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fmt_duration_zero() {
        assert_eq!(super::fmt_duration(0), "00:00");
    }

    #[test]
    fn fmt_duration_90s() {
        assert_eq!(super::fmt_duration(90), "01:30");
    }

    #[test]
    fn title_recording_shows_time() {
        let t = tray_title(RecordingState::Recording, 65);
        assert!(t.contains("01:05"), "got: {t}");
    }
}
