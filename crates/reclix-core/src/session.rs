//! Session model.
//!
//! A *session* owns the state machine, all segments, audio/video source config,
//! and the path to the final output file.  It is serialised as `session.json`
//! inside the session directory so a crashed session can (in future) be
//! recovered.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    error::{ReclixError, Result},
    segment::Segment,
    state::{RecordingState, StateMachine, StateEvent},
};

/// Video source type for the session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VideoSource {
    /// Full-screen (single monitor) via XDG ScreenCast portal.
    FullScreen,
    // V0.2+: Region, Window
}

/// Audio configuration for the session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    pub mic_enabled: bool,
    pub system_audio_enabled: bool,  // V0.2+
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            mic_enabled: true,
            system_audio_enabled: false,
        }
    }
}

/// Top-level session record — serialised to `session.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: Uuid,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,

    /// Directory where all segment files and this metadata live.
    pub session_dir: PathBuf,

    /// Final MP4 output path (set when FINISHING begins).
    pub output_file: Option<PathBuf>,

    pub video_source: VideoSource,
    pub audio: AudioConfig,

    pub segments: Vec<Segment>,

    #[serde(skip)]
    pub state_machine: StateMachine,
}

impl Session {
    /// Create a new session rooted at `session_dir`.
    /// The directory must already exist.
    pub fn new(session_dir: PathBuf, video_source: VideoSource, audio: AudioConfig) -> Self {
        Self {
            id: Uuid::new_v4(),
            started_at: Utc::now(),
            ended_at: None,
            session_dir,
            output_file: None,
            video_source,
            audio,
            segments: Vec::new(),
            state_machine: StateMachine::new(),
        }
    }

    // ── State delegators ─────────────────────────────────────────────────────

    pub fn state(&self) -> RecordingState {
        self.state_machine.state()
    }

    pub fn apply(&mut self, event: StateEvent) -> Result<RecordingState> {
        self.state_machine.transition(event)
    }

    // ── Segment helpers ───────────────────────────────────────────────────────

    /// Open a new segment (call after RECORDING state is entered).
    pub fn open_segment(&mut self) -> &Segment {
        let index = self.segments.len() as u32;
        let seg = Segment::new(index);
        self.segments.push(seg);
        self.segments.last().unwrap()
    }

    /// Close the currently open segment (call on PAUSE or FINISH).
    pub fn close_current_segment(&mut self) {
        if let Some(seg) = self.segments.iter_mut().rev().find(|s| s.is_open()) {
            seg.close();
        }
    }

    /// Mutable reference to the currently open segment, if any.
    pub fn current_segment_mut(&mut self) -> Option<&mut Segment> {
        self.segments.iter_mut().rev().find(|s| s.is_open())
    }

    /// Total active recording time across all closed + open segments (seconds).
    pub fn total_recording_secs(&self) -> u64 {
        self.segments
            .iter()
            .filter(|s| s.active)
            .filter_map(|s| {
                if s.is_open() {
                    let now = Utc::now();
                    Some(now.signed_duration_since(s.started_at).num_seconds().max(0) as u64)
                } else {
                    s.duration_secs()
                }
            })
            .sum()
    }

    /// All closed, active segments ready for export.
    pub fn exportable_segments(&self) -> Vec<&Segment> {
        self.segments
            .iter()
            .filter(|s| s.active && !s.is_open())
            .collect()
    }

    // ── Persistence ───────────────────────────────────────────────────────────

    /// Write `session.json` into `self.session_dir`.
    pub fn save(&self) -> Result<()> {
        let path = self.session_dir.join("session.json");
        let json = serde_json::to_string_pretty(self)
            .map_err(ReclixError::MetadataParse)?;
        std::fs::write(&path, json)
            .map_err(ReclixError::MetadataWrite)?;
        Ok(())
    }

    /// Load a session from a `session.json` file.
    pub fn load(session_json: &Path) -> Result<Self> {
        let data = std::fs::read_to_string(session_json)
            .map_err(ReclixError::MetadataRead)?;
        serde_json::from_str(&data)
            .map_err(ReclixError::MetadataParse)
    }
}

// ── Session directory naming ──────────────────────────────────────────────────

/// Return a timestamped session directory name, e.g. `reclix_20240924_143022`.
pub fn session_dir_name() -> String {
    chrono::Local::now().format("reclix_%Y%m%d_%H%M%S").to_string()
}

/// Ensure `~/Videos/Reclix/` exists and return its path.
pub fn ensure_output_dir() -> Result<PathBuf> {
    let videos = dirs::video_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from("~")).join("Videos"));
    let dir = videos.join("Reclix");
    std::fs::create_dir_all(&dir).map_err(ReclixError::OutputDirCreate)?;
    Ok(dir)
}

// ─── tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn make_session() -> (TempDir, Session) {
        let tmp = TempDir::new().unwrap();
        let sess = Session::new(
            tmp.path().to_path_buf(),
            VideoSource::FullScreen,
            AudioConfig::default(),
        );
        (tmp, sess)
    }

    #[test]
    fn new_session_is_idle() {
        let (_tmp, sess) = make_session();
        assert_eq!(sess.state(), RecordingState::Idle);
    }

    #[test]
    fn segment_lifecycle() {
        let (_tmp, mut sess) = make_session();
        sess.apply(StateEvent::Start).unwrap();
        sess.apply(StateEvent::Ready).unwrap();

        sess.open_segment();
        assert_eq!(sess.segments.len(), 1);
        assert!(sess.segments[0].is_open());

        sess.close_current_segment();
        assert!(!sess.segments[0].is_open());

        // Second segment
        sess.open_segment();
        assert_eq!(sess.segments.len(), 2);
        assert!(sess.segments[1].is_open());
    }

    #[test]
    fn exportable_segments_excludes_open() {
        let (_tmp, mut sess) = make_session();
        sess.apply(StateEvent::Start).unwrap();
        sess.apply(StateEvent::Ready).unwrap();
        sess.open_segment();
        // open segment — not exportable yet
        assert_eq!(sess.exportable_segments().len(), 0);
        sess.close_current_segment();
        // now closed — exportable
        assert_eq!(sess.exportable_segments().len(), 1);
    }

    #[test]
    fn save_and_load_roundtrip() {
        let (tmp, sess) = make_session();
        sess.save().unwrap();
        let path = tmp.path().join("session.json");
        assert!(path.exists());
        let loaded = Session::load(&path).unwrap();
        assert_eq!(loaded.id, sess.id);
        assert_eq!(loaded.segments.len(), 0);
    }

    #[test]
    fn session_dir_name_format() {
        let name = session_dir_name();
        assert!(name.starts_with("reclix_"));
        assert_eq!(name.len(), "reclix_20240924_143022".len());
    }
}
