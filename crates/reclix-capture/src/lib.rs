//! reclix-capture — XDG ScreenCast portal + PipeWire screen capture.
//!
//! # Architecture
//!
//! ```text
//!  ┌──────────────────────┐   fd    ┌────────────────────────┐
//!  │ XDG ScreenCast portal│ ──────► │  PipeWire capture loop │
//!  │     (ashpd)          │         │   (dedicated thread)   │
//!  └──────────────────────┘         └──────────┬─────────────┘
//!                                              │ raw video frames
//!                                              ▼
//!                                    tokio::sync::mpsc sender
//!                                    (consumed by reclix-encode)
//! ```
//!
//! The PipeWire main loop **must** run on its own OS thread — it uses a
//! `MainLoop` which is not `Send`.  We launch it with `std::thread::spawn`
//! and communicate via channels.

pub mod portal;
pub mod pipewire_capture;
pub mod frame;

pub use frame::{RawFrame, FrameFormat};
pub use portal::ScreenCastSource;
pub use pipewire_capture::{CaptureHandle, start_capture};
