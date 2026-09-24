//! Segment tracking.
//!
//! A recording session consists of one or more *segments*.  Each pause/resume
//! cycle closes the current segment and opens a new one.  Only active segments
//! are included in the final export; paused time is discarded.

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A single contiguous capture interval.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Segment {
    /// Unique ID — used to name the raw video file on disk.
    pub id: Uuid,
    /// Segment sequence number within the session (0-based).
    pub index: u32,
    /// Wall-clock time this segment started.
    pub started_at: DateTime<Utc>,
    /// Wall-clock time this segment ended (None while active).
    pub ended_at: Option<DateTime<Utc>>,
    /// Path to the raw video file written by the encoder.
    /// Relative to the session directory.
    pub video_file: PathBuf,
    /// Path to the raw audio file (may be absent if mic was disabled).
    pub audio_file: Option<PathBuf>,
    /// Whether this segment is included in the final export.
    pub active: bool,
}

impl Segment {
    /// Create a new open segment.
    pub fn new(index: u32) -> Self {
        let id = Uuid::new_v4();
        Self {
            id,
            index,
            started_at: Utc::now(),
            ended_at: None,
            video_file: PathBuf::from(format!("segment_{index:03}_{id}.mkv")),
            audio_file: None,
            active: true,
        }
    }

    /// Mark the segment as finished.
    pub fn close(&mut self) {
        self.ended_at = Some(Utc::now());
    }

    /// Duration in whole seconds; `None` while segment is still open.
    pub fn duration_secs(&self) -> Option<u64> {
        let end = self.ended_at?;
        let dur = end.signed_duration_since(self.started_at);
        Some(dur.num_seconds().max(0) as u64)
    }

    /// Is this segment still open (being recorded)?
    pub fn is_open(&self) -> bool {
        self.ended_at.is_none()
    }
}

// ─── tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_segment_is_open() {
        let s = Segment::new(0);
        assert!(s.is_open());
        assert!(s.ended_at.is_none());
        assert!(s.active);
    }

    #[test]
    fn closed_segment_has_duration() {
        let mut s = Segment::new(0);
        s.close();
        assert!(!s.is_open());
        // duration ≥ 0
        assert!(s.duration_secs().is_some());
    }

    #[test]
    fn video_file_contains_index() {
        let s = Segment::new(7);
        assert!(s.video_file.to_string_lossy().contains("segment_007_"));
    }
}
