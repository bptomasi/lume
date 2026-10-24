use super::*;
use image::Rgb;

#[test]
fn exact_indexing_preserves_all_256_colors_and_pixel_positions() -> Result<()> {
    let image = RgbImage::from_fn(256, 8, |x, y| {
        let color = ((x + y) % 256) as u8;
        Rgb([color, 255 - color, color / 2])
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
