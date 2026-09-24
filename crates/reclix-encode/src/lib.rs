//! reclix-encode — FFmpeg-based encoding pipeline.
//!
//! # Design
//!
//! We shell out to the system `ffmpeg` binary rather than link against
//! libavcodec.  This avoids GPL/LGPL linking concerns, lets the user benefit
//! from a distro-patched FFmpeg, and keeps the build simple (no C headers).
//!
//! ## Per-segment encoding
//!
//! Each [`Segment`] gets its own FFmpeg process writing a `.mkv` container.
//! The service layer opens a new encoder when recording starts or resumes,
//! and closes it (sends EOF to stdin) when the segment ends on pause/finish.
//!
//! ## Encoder selection (VA-API → x264 fallback)
//!
//! On startup [`EncoderPipeline::probe_vaapi`] tries to open `/dev/dri/renderD128`
//! and runs a tiny FFmpeg probe.  If VA-API is unavailable we fall back to
//! `libx264`.  The choice is cached for the lifetime of the service.
//!
//! ## Final export
//!
//! [`EncoderPipeline::concat_segments`] builds an ffmpeg concat demuxer file
//! from all exportable segments and produces the final MP4.

pub mod pipeline;
pub mod vaapi;

pub use pipeline::{EncoderPipeline, SegmentEncoder, EncoderConfig};
pub use vaapi::probe_vaapi;
