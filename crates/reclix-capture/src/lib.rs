//! reclix-capture — XDG ScreenCast portal + PipeWire screen capture.
//!
//! The `pipewire-capture` feature (default) pulls in `libpipewire-0.3-dev`.
//! Build without it (`--no-default-features`) to compile on systems without
//! the PipeWire headers installed (the stubs will panic at runtime).

pub mod frame;

#[cfg(feature = "pipewire-capture")]
pub mod portal;

#[cfg(feature = "pipewire-capture")]
pub mod pipewire_capture;

pub use frame::{FrameFormat, RawFrame};

#[cfg(feature = "pipewire-capture")]
pub use pipewire_capture::{start_capture, CaptureHandle};

#[cfg(feature = "pipewire-capture")]
pub use portal::ScreenCastSource;

// ── Stub types when feature is disabled ──────────────────────────────────────

#[cfg(not(feature = "pipewire-capture"))]
pub struct CaptureHandle;

#[cfg(not(feature = "pipewire-capture"))]
impl CaptureHandle {
    pub fn pause(&self) {}
    pub fn resume(&self) {}
}

#[cfg(not(feature = "pipewire-capture"))]
pub fn start_capture(
    _fd: std::os::fd::OwnedFd,
    _node_id: u32,
    _fps: u32,
    _frame_tx: tokio::sync::mpsc::Sender<RawFrame>,
) -> anyhow::Result<CaptureHandle> {
    anyhow::bail!("reclix-capture compiled without pipewire-capture feature")
}
