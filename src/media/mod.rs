//! Bounded article thumbnails, independent of text layout and terminal I/O.
//!
//! The worker owns HTTP, disk cache, decoding and halfblock encoding. The UI
//! admits one request at a time and paints immutable cells. Fixed reader slots
//! prevent late images from moving text, selections or annotation anchors.

pub mod cache;
mod decode;
mod fonts;
mod svg;
mod worker;

pub use decode::Decoder;
pub(crate) use worker::{Completion, Worker};

use ratatui::buffer::Buffer;

/// Maximum compressed bytes for one raster image, including cached files.
pub const MAX_BYTES: usize = 8 * 1024 * 1024;

/// Exact identity of a prepared thumbnail, including dimensions and matte.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageKey {
    /// Validated absolute HTTP(S) URL.
    pub url: String,
    /// Maximum columns, clamped to 1–160 by the decoder.
    pub width: u16,
    /// Maximum rows, clamped to 1–48 by the decoder.
    pub height: u16,
    /// Opaque sRGB background used for transparent pixels.
    pub background: [u8; 3],
}

/// A ready-to-paint thumbnail; no original-size pixel buffer is retained.
pub struct Thumbnail {
    /// Aspect-preserving terminal cells, at most the requested area.
    pub cells: Buffer,
    /// Original raster or logical SVG dimensions, before thumbnailing.
    pub dimensions: (u32, u32),
    /// Cache write failure after successful rendering, if any.
    pub cache_warning: Option<String>,
}
