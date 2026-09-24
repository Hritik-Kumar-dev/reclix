//! Raw frame types passed from the PipeWire capture thread to the encoder.

use std::time::Duration;

/// Pixel format of a raw captured frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameFormat {
    /// 4 bytes per pixel, BGRA order (most common PipeWire DMA-BUF format).
    Bgra,
    /// 4 bytes per pixel, RGBA order.
    Rgba,
    /// NV12 planar YUV — preferred by VA-API.
    Nv12,
}

impl FrameFormat {
    /// FFmpeg pixel format string for this format.
    pub fn ffmpeg_pix_fmt(self) -> &'static str {
        match self {
            FrameFormat::Bgra => "bgra",
            FrameFormat::Rgba => "rgba",
            FrameFormat::Nv12 => "nv12",
        }
    }
}

/// A single captured video frame.
///
/// Frames are cloned cheaply via `Arc` when multiple consumers exist.
#[derive(Debug, Clone)]
pub struct RawFrame {
    /// Monotonic presentation timestamp relative to session start.
    pub pts: Duration,
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Bytes per row (may be > width * bytes_per_pixel due to alignment).
    pub stride: u32,
    pub format: FrameFormat,
    /// Raw pixel data.
    pub data: Vec<u8>,
}

impl RawFrame {
    /// Size in bytes expected for a packed (stride == width * bpp) frame.
    pub fn packed_size(width: u32, height: u32, format: FrameFormat) -> usize {
        let bpp: usize = match format {
            FrameFormat::Bgra | FrameFormat::Rgba => 4,
            FrameFormat::Nv12 => 1, // Y plane; UV plane adds height/2 more rows
        };
        (width as usize) * (height as usize) * bpp
    }
}
