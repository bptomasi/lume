use super::*;
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
fn palette_compaction_preserves_every_displayed_edge_color() {
    let colors: Vec<_> = (0_u8..=255).flat_map(|n| [n, n, n]).collect();
    let indices: Vec<_> = (0_u8..=255).chain((0_u8..=255).rev()).collect();
    let (palette, mapped) = compact(colors.clone(), indices.clone());
    assert_eq!(palette.len() / 3, 101);
    assert_eq!(mapped.len(), indices.len());
    for (old, new) in indices.into_iter().zip(mapped) {
        let old = usize::from(old) * 3;
        let new = usize::from(new) * 3;
        let before = [colors[old], colors[old + 1], colors[old + 2]];
        let after = [palette[new], palette[new + 1], palette[new + 2]];
        assert_eq!(terminal_color(before), terminal_color(after));
    }
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

#[test]
fn antialiased_colors_that_fit_sixel_are_indexed_without_quantization()
-> Result<()> {
    let image = RgbImage::from_fn(256, 8, |x, y| {
        let n = x as u8;
        Rgb([n.saturating_add((y % 2) as u8), n, n])
    });
    let unique = image
        .pixels()
        .map(|p| p.0)
        .collect::<std::collections::HashSet<_>>();
    assert!(unique.len() > 256);
    assert!(exact_colors(&image).is_some());
    let (palette, indices) = index(&image)?;
    for (pixel, index) in image.pixels().zip(indices) {
        let start = usize::from(index) * 3;
        assert_eq!(
            terminal_color(pixel.0),
            terminal_color([
                palette[start],
                palette[start + 1],
                palette[start + 2]
            ])
        );
    }
    Ok(())
}
