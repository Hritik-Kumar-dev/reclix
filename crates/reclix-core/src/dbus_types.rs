//! Shared type definitions that are serialised over the D-Bus interface.
//!
//! These are plain Rust structs that implement `serde::{Serialize, Deserialize}`
//! so both the service (server) and CLI (client) can share them without
//! pulling in `zbus` as a hard dependency of reclix-core.

use serde::{Deserialize, Serialize};

use crate::state::RecordingState;

/// Snapshot of service state returned by `GetStatus`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceStatus {
    pub state:            RecordingState,
    /// Total active recording time in seconds.
    pub recording_secs:   u64,
    /// Current segment index (0-based), `None` when IDLE.
    pub segment_index:    Option<u32>,
    /// Whether the microphone is currently enabled.
    pub mic_enabled:      bool,
    /// Whether the camera overlay is active (V0.3+, always false in V0.1).
    pub camera_enabled:   bool,
    /// Path of the final output file once COMPLETED, otherwise `None`.
    pub output_file:      Option<String>,
}

/// Commands accepted over D-Bus `SendCommand`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceCommand {
    Start,
    PauseResume,
    Finish,
    ToggleMic,
    ToggleCamera,
    ToggleFocus,
    ToggleSystemAudio,
}
