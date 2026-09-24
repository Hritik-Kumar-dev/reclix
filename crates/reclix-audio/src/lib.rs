//! reclix-audio — PipeWire microphone capture.
//!
//! The `pipewire-audio` feature (default) pulls in `libpipewire-0.3-dev`.
//! Build without it (`--no-default-features`) for systems lacking PipeWire headers.

pub mod audio_frame;

#[cfg(feature = "pipewire-audio")]
pub mod mic_capture;

pub use audio_frame::{AudioFrame, SampleFormat};

#[cfg(feature = "pipewire-audio")]
pub use mic_capture::{start_mic_capture, MicHandle};

// ── Stubs ─────────────────────────────────────────────────────────────────────

#[cfg(not(feature = "pipewire-audio"))]
pub struct MicHandle;

#[cfg(not(feature = "pipewire-audio"))]
impl MicHandle {
    pub fn mute(&self) {}
    pub fn unmute(&self) {}
    pub fn is_active(&self) -> bool { false }
}

#[cfg(not(feature = "pipewire-audio"))]
pub fn start_mic_capture(
    _sample_rate: u32,
    _channels: u32,
    _frame_tx: tokio::sync::mpsc::Sender<AudioFrame>,
) -> anyhow::Result<MicHandle> {
    anyhow::bail!("reclix-audio compiled without pipewire-audio feature")
}
