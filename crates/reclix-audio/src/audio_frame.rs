//! Audio sample types.

use std::time::Duration;

/// PCM sample format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleFormat {
    /// Signed 16-bit little-endian, interleaved.
    S16Le,
    /// 32-bit float little-endian, interleaved.
    F32Le,
}

impl SampleFormat {
    /// FFmpeg sample format string.
    pub fn ffmpeg_fmt(self) -> &'static str {
        match self {
            SampleFormat::S16Le => "s16le",
            SampleFormat::F32Le => "f32le",
        }
    }

    /// Bytes per sample.
    pub fn bytes_per_sample(self) -> usize {
        match self {
            SampleFormat::S16Le => 2,
            SampleFormat::F32Le => 4,
        }
    }
}

/// A chunk of interleaved PCM audio.
#[derive(Debug, Clone)]
pub struct AudioFrame {
    /// Presentation timestamp relative to session start.
    pub pts: Duration,
    pub format: SampleFormat,
    pub channels: u32,
    pub sample_rate: u32,
    /// Raw interleaved PCM bytes.
    pub data: Vec<u8>,
}

impl AudioFrame {
    /// Number of frames (channel-agnostic sample points) in this chunk.
    pub fn frame_count(&self) -> usize {
        let bytes_per_frame = self.format.bytes_per_sample() * self.channels as usize;
        self.data.len().checked_div(bytes_per_frame).unwrap_or(0)
    }
}
