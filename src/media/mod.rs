//! Bounded article thumbnails, independent of text layout and terminal I/O.
//!
//! The worker owns HTTP, disk cache, decoding and presentation encoding. The UI
//! admits one request at a time and paints immutable cells or plans prepared
//! pixel placements. Fixed reader slots preserve text and annotation anchors.

pub mod cache;
mod decode;
mod fonts;
mod geometry;
mod sixel;
mod svg;
mod worker;

pub use decode::Decoder;
pub use sixel::SixelRows;
pub(crate) use worker::{Completion, Worker};

use ratatui::buffer::Buffer;

/// Maximum compressed bytes for one raster image, including cached files.
pub const MAX_BYTES: usize = 8 * 1024 * 1024;

/// Session presentation for prepared inline images and the image inspector.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ImageProtocol {
    /// Portable character approximation, including Alacritty.
    #[default]
    Blocks,
    /// DEC Sixel, requiring a supporting terminal.
    Sixel,
    /// iTerm2 inline images, also supported by WezTerm.
    Iterm2,
}

impl ImageProtocol {
    /// Human-readable presentation name for status and inspector chrome.
    pub fn label(self) -> &'static str {
        match self {
            Self::Blocks => "Blocks",
            Self::Sixel => "Sixel pixels",
            Self::Iterm2 => "iTerm2 pixels",
        }
    }
}

/// Exact source-based inspection settings; never zoom a cached thumbnail.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageDetail {
    /// Integer magnification of the fitted source, from one through eight.
    pub zoom: u8,
    /// Horizontal/vertical position within available overflow, in 0–1000 units.
    pub pan: [u16; 2],
    /// Explicit native encoder or portable blocks.
    pub protocol: ImageProtocol,
    /// Actual cell width/height in pixels; ignored by the block encoder.
    pub cell_pixels: (u16, u16),
    /// Fitted inline row budget; absent in the inspector.
    pub slice: Option<ImageSlice>,
}

/// Requested source rows within a fitted inline image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageSlice {
    /// Rows omitted from the top of the fitted image.
    pub first_row: u16,
    /// Maximum visible image rows.
    pub rows: u16,
}

/// Exact identity of a prepared thumbnail, including dimensions and matte.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageKey {
    /// Validated absolute HTTP(S) URL.
    pub url: String,
    /// Maximum columns; the decoder rejects values outside 1–160.
    pub width: u16,
    /// Maximum rows; the decoder rejects values outside 1–48.
    pub height: u16,
    /// Opaque sRGB background used for transparent pixels.
    pub background: [u8; 3],
    /// Present for source-based inspection or native inline pixels.
    pub detail: Option<ImageDetail>,
}

/// A ready-to-paint thumbnail; no original-size pixel buffer is retained.
pub struct Thumbnail {
    /// Aspect-preserving area; native encodings leave these cells blank.
    pub cells: Buffer,
    /// Original raster or logical SVG dimensions, before thumbnailing.
    pub dimensions: (u32, u32),
    /// Cache write failure after successful rendering, if any.
    pub cache_warning: Option<String>,
    /// Worker-encoded native pixels; painted only in the image inspector.
    pub native: Option<ratatui_image::protocol::Protocol>,
    /// Worker-encoded cell-height Sixel strips for clipped inline drawing.
    pub rows: Option<SixelRows>,
}
