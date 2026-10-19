//! Scroll crops reuse fitted pixels on the presentation worker, never the UI.
//!
//! Full images remain independent of scroll position. Four decoded sources and
//! eight encoded crops each have a 16 MiB retention budget. Frame references
//! keep published PNGs alive through replacement and helper shutdown. Weak source
//! identities avoid retaining original files outside their owner's cache budget.

use std::{
    collections::VecDeque,
    ops::Range,
    sync::{Arc, Weak},
};

use anyhow::{Result, ensure};
use image::DynamicImage;

use crate::media::PreparedPng;

use super::Placement;

const CACHE_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Default)]
pub(super) struct Projection {
    sources: VecDeque<Source>,
    crops: VecDeque<Crop>,
}

struct Source {
    owner: Weak<PreparedPng>,
    pixels: DynamicImage,
}

struct Crop {
    source: Weak<PreparedPng>,
    rows: Range<u32>,
    png: Arc<PreparedPng>,
}

impl Projection {
    pub(super) fn prepare(
        &mut self,
        mut placement: Placement,
    ) -> Result<Placement> {
        let Some(rows) = placement.crop.take() else {
            return Ok(placement);
        };
        if let Some(crop) = self.crops.iter().find(|crop| {
            crop.source.ptr_eq(&Arc::downgrade(&placement.image))
                && crop.rows == rows
        }) {
            placement.image = crop.png.clone();
            return Ok(placement);
        }
        let pixels = self.pixels(&placement.image)?;
        let end = rows.end.min(pixels.height());
        ensure!(rows.start < end, "Overlay crop has no visible pixels");
        let image =
            pixels.crop_imm(0, rows.start, pixels.width(), end - rows.start);
        let png = Arc::new(PreparedPng::new(&image)?);
        self.crops.push_front(Crop {
            source: Arc::downgrade(&placement.image),
            rows,
            png: png.clone(),
        });
        self.trim();
        placement.image = png;
        Ok(placement)
    }

    fn pixels(&mut self, owner: &Arc<PreparedPng>) -> Result<&DynamicImage> {
        if let Some(index) = self
            .sources
            .iter()
            .position(|source| source.owner.ptr_eq(&Arc::downgrade(owner)))
        {
            return Ok(&self.sources[index].pixels);
        }
        // The input is an owned, already bounded PNG, never publisher bytes.
        let pixels = image::open(owner.path())?;
        ensure!(
            u64::from(pixels.width()) * u64::from(pixels.height()) <= 4_000_000,
            "Prepared overlay exceeds its pixel budget"
        );
        self.sources.push_front(Source {
            owner: Arc::downgrade(owner),
            pixels: DynamicImage::ImageRgba8(pixels.to_rgba8()),
        });
        self.trim();
        Ok(&self.sources[0].pixels)
    }

    fn trim(&mut self) {
        self.sources.truncate(4);
        let mut bytes = 0;
        self.sources.retain(|source| {
            bytes += u64::from(source.pixels.width())
                * u64::from(source.pixels.height())
                * 4;
            bytes <= CACHE_BYTES
        });
        self.crops.truncate(8);
        bytes = 0;
        self.crops.retain(|crop| {
            bytes += crop.png.bytes();
            bytes <= CACHE_BYTES
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::{Decoder, ImageDetail, ImageKey, ImageProtocol};
    use anyhow::Context;
    use ratatui::layout::Rect;

    const SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="120"><path fill="red" d="M0 0h100v60H0z"/><path fill="blue" d="M0 60h100v60H0z"/></svg>"#;

    fn placement() -> Result<Placement> {
        let key = ImageKey {
            url: "https://example.org/colors.svg".into(),
            width: 10,
            height: 6,
            background: [0, 0, 0],
            detail: Some(ImageDetail {
                zoom: 1,
                pan: [0, 0],
                protocol: ImageProtocol::Overlay,
                cell_pixels: (10, 20),
                slice: None,
            }),
        };
        Ok(Placement {
            id: "colors".into(),
            area: Rect::new(2, 3, 10, 2),
            image: Decoder::default()
                .render(SVG, &key)?
                .overlay
                .context("Source pixels")?,
            crop: Some(60..100),
        })
    }

    #[test]
    fn crops_preserve_source_pixels_and_reuse_the_prepared_file() -> Result<()>
    {
        let mut projection = Projection::default();
        let source = placement()?;
        let bottom = projection.prepare(source.clone())?;
        let pixels = image::open(bottom.image.path())?.to_rgb8();
        assert_eq!(pixels.dimensions(), (100, 40));
        assert!(pixels.pixels().all(|pixel| pixel.0 == [0, 0, 255]));
        let again = projection.prepare(source.clone())?;
        assert_eq!(again.image.path(), bottom.image.path());
        assert!(again.crop.is_none());

        let top = projection.prepare(Placement {
            crop: Some(0..40),
            ..source.clone()
        })?;
        let pixels = image::open(top.image.path())?.to_rgb8();
        assert!(pixels.pixels().all(|pixel| pixel.0 == [255, 0, 0]));
        assert_eq!(image::open(source.image.path())?.height(), 120);
        assert_eq!(projection.sources.len(), 1);
        assert_eq!(projection.crops.len(), 2);
        Ok(())
    }

    #[test]
    fn invalid_crops_fail_and_retained_crops_remain_bounded() -> Result<()> {
        let mut projection = Projection::default();
        let source = placement()?;
        for start in 0..20 {
            projection.prepare(Placement {
                crop: Some(start..start + 20),
                ..source.clone()
            })?;
        }
        assert_eq!(projection.crops.len(), 8);
        // Deliberately malformed bounds qualify rejection, not iteration.
        let reversed = Range { start: 90, end: 20 };
        for rows in [120..140, 60..60, reversed] {
            assert!(
                projection
                    .prepare(Placement {
                        crop: Some(rows),
                        ..source.clone()
                    })
                    .is_err()
            );
        }
        let clipped = projection.prepare(Placement {
            crop: Some(100..140),
            ..source
        })?;
        assert_eq!(image::open(clipped.image.path())?.height(), 20);
        Ok(())
    }
}
