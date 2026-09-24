//! reclix-audio — PipeWire microphone capture.
//!
//! Runs the PipeWire audio loop on its own OS thread (same reason as video:
//! `MainLoop` is not `Send`).  Raw S16LE / F32LE PCM samples are forwarded to
//! the encoder via a `tokio::sync::mpsc` channel.
//!
//! # Mute / unmute
//! We do NOT renegotiate the PipeWire stream; instead `MicHandle::mute` /
//! `unmute` set an atomic flag and the process callback discards samples when
//! muted.  This gives instant response with zero latency cost.

pub mod mic_capture;
pub mod audio_frame;

pub use audio_frame::{AudioFrame, SampleFormat};
pub use mic_capture::{MicHandle, start_mic_capture};
