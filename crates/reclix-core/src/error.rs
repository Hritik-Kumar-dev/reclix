//! Reclix error types.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ReclixError {
    // ── State machine ────────────────────────────────────────────────────────
    #[error("invalid state transition: {from} → {to}")]
    InvalidTransition { from: String, to: String },

    #[error("operation requires state {required}, current state is {current}")]
    WrongState { required: String, current: String },

    // ── Session / IO ─────────────────────────────────────────────────────────
    #[error("session directory could not be created: {0}")]
    SessionDirCreate(#[source] std::io::Error),

    #[error("session metadata write failed: {0}")]
    MetadataWrite(#[source] std::io::Error),

    #[error("session metadata read failed: {0}")]
    MetadataRead(#[source] std::io::Error),

    #[error("session metadata parse failed: {0}")]
    MetadataParse(#[source] serde_json::Error),

    // ── Capture / Portal ─────────────────────────────────────────────────────
    #[error("screen-cast portal denied or unavailable: {0}")]
    PortalDenied(String),

    #[error("PipeWire capture error: {0}")]
    PipeWire(String),

    // ── Audio ────────────────────────────────────────────────────────────────
    #[error("microphone unavailable: {0}")]
    MicUnavailable(String),

    // ── Encoding ─────────────────────────────────────────────────────────────
    #[error("encoder unavailable (tried VA-API and libx264): {0}")]
    EncoderUnavailable(String),

    #[error("FFmpeg process error: {0}")]
    FfmpegProcess(String),

    #[error("output directory could not be created: {0}")]
    OutputDirCreate(#[source] std::io::Error),

    // ── D-Bus ────────────────────────────────────────────────────────────────
    #[error("D-Bus error: {0}")]
    DBus(String),

    // ── Generic ──────────────────────────────────────────────────────────────
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, ReclixError>;
