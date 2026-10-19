//! Fit and crop geometry is shared by raster pixels and vector transforms.

use anyhow::{Result, ensure};

use super::{ImageKey, ImageProtocol};

pub(super) struct Geometry {
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) scale: f32,
    pub(super) offset: [f32; 2],
}

impl Geometry {
    pub(super) fn new(source: (f32, f32), key: &ImageKey) -> Self {
        let (cell_width, cell_height) = cell_pixels(key);
        let target_width = u32::from(key.width) * u32::from(cell_width);
        let target_height = u32::from(key.height) * u32::from(cell_height);
        let zoom = key.detail.as_ref().map_or(1, |detail| detail.zoom);
        let scale = (target_width as f32 / source.0)
            .min(target_height as f32 / source.1)
            * f32::from(zoom);
        let full_width = (source.0 * scale).ceil().max(1.0);
        let full_height = (source.1 * scale).ceil().max(1.0);
        let width = target_width.min(full_width as u32);
        let height = target_height.min(full_height as u32);
        let pan = key.detail.as_ref().map_or([0, 0], |detail| detail.pan);
        Self {
            width,
            height,
            scale,
            offset: [
                (full_width - width as f32) * f32::from(pan[0]) / 1000.0,
                (full_height - height as f32) * f32::from(pan[1]) / 1000.0,
            ],
        }
    }
}

pub(super) fn validate(key: &ImageKey) -> Result<()> {
    ensure!(
        (1..=160).contains(&key.width) && (1..=48).contains(&key.height),
        "Image viewport exceeds 160 columns or 48 rows"
    );
    if let Some(detail) = &key.detail {
        if let Some(slice) = &detail.slice {
            ensure!(
                slice.rows > 0
                    && u32::from(slice.first_row) + u32::from(slice.rows)
                        <= u32::from(key.height),
                "Image slice exceeds its viewport"
            );
        }
        ensure!((1..=8).contains(&detail.zoom), "Image zoom must be 1–8");
        ensure!(
            detail.pan.iter().all(|value| *value <= 1000),
            "Image pan exceeds its source bounds"
        );
        ensure!(
            (1..=64).contains(&detail.cell_pixels.0)
                && (1..=128).contains(&detail.cell_pixels.1),
            "Invalid terminal pixel cell size"
        );
    }
    let (width, height) = cell_pixels(key);
    let pixels = u64::from(key.width)
        * u64::from(key.height)
        * u64::from(width)
        * u64::from(height);
    ensure!(
        pixels <= 4_000_000,
        "Image viewport exceeds 4 million pixels"
    );
    Ok(())
}

pub(super) fn cell_pixels(key: &ImageKey) -> (u16, u16) {
    key.detail
        .as_ref()
        .filter(|detail| detail.protocol != ImageProtocol::Blocks)
        .map_or((1, 2), |detail| detail.cell_pixels)
}
