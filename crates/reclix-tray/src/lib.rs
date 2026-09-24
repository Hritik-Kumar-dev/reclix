//! reclix-tray — StatusNotifierItem system tray via `ksni` 0.3.
//!
//! The tray icon reflects recording state:
//!   IDLE      → media-record icon, tooltip "Reclix — idle"
//!   RECORDING → media-record-symbolic, tooltip "🔴 mm:ss"
//!   PAUSED    → media-playback-pause-symbolic, tooltip "⏸  mm:ss"
//!   STARTING / FINISHING → content-loading-symbolic
//!   COMPLETED → emblem-ok-symbolic
//!
//! State is updated by writing to the `Arc<Mutex<TrayState>>` returned by
//! `spawn_tray()` and then calling `handle.update(|_| true)` to force a
//! menu/icon refresh.

pub mod icon;
pub mod indicator;

pub use indicator::{ReclixTray, TrayCommand, TrayState, spawn_tray};
