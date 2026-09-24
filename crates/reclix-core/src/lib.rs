//! reclix-core — state machine, session model, segment tracking.
//!
//! This crate has **no** async runtime dependency and no system libs so it can
//! be unit-tested in any environment.

pub mod error;
pub mod segment;
pub mod session;
pub mod state;
pub mod config;
pub mod dbus_types;
