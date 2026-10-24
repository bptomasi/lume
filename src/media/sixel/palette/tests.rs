use super::*;
use anyhow::Context;
use image::Rgb;

#[test]
fn exact_indexing_preserves_all_256_colors_and_pixel_positions() -> Result<()> {
    let image = RgbImage::from_fn(256, 8, |x, y| {
        let color = ((x + y) % 256) as u8;
        Rgb([(color % 16) * 17, (color / 16) * 17, 128])
    });
    let (palette, indices) = index(&image)?;
    assert_eq!(palette.len(), 256 * 3);
    assert_eq!(indices.len(), 256 * 8);
    for (pixel, index) in image.pixels().zip(indices) {
        let start = usize::from(index) * 3;
        assert_eq!(&palette[start..start + 3], &pixel.0);
    }
    Ok(())
}

#[test]
fn antialiased_edges_keep_every_representable_color_without_quantization()
-> Result<()> {
    let image = RgbImage::from_fn(256, 2, |x, y| {
        let value = x as u8;
        Rgb([value.saturating_add(y as u8), value, value])
    });
    let raw: std::collections::BTreeSet<_> =
        image.pixels().map(|p| p.0).collect();
    assert!(raw.len() > 256);
    let (palette, indices) =
        exact_colors(&image).context("Representable edges")?;
    assert!(palette.len() / 3 <= 256);
    for (pixel, index) in image.pixels().zip(indices) {
        let start = usize::from(index) * 3;
        let encoded = [palette[start], palette[start + 1], palette[start + 2]];
        assert_eq!(terminal_color(encoded), terminal_color(pixel.0));
    }
    Ok(())
}

#[test]
fn richer_images_use_bounded_quantization_without_truncating_pixels()
-> Result<()> {
    let image = RgbImage::from_fn(128, 64, |x, y| {
        Rgb([(x * 2) as u8, (y * 4) as u8, ((x + y) % 256) as u8])
    });
    assert!(exact_colors(&image).is_none());
    let (palette, indices) = index(&image)?;
    assert!(!palette.is_empty() && palette.len() <= 768);
    assert_eq!(indices.len(), 128 * 64);
    let mut squared_error = 0_u64;
    for (pixel, index) in image.pixels().zip(indices) {
        let start = usize::from(index) * 3;
        for (actual, original) in palette[start..start + 3].iter().zip(pixel.0)
        {
            squared_error +=
                (i64::from(*actual) - i64::from(original)).pow(2) as u64;
        }
    }
    // A smooth RGB gradient must retain color detail after 256-color diffusion.
    assert!(squared_error / (128 * 64 * 3) < 100);
    Ok(())
}
