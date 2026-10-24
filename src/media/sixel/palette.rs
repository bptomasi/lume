//! Keep exact diagram colors; quantize richer images once across all strips.
use anyhow::Result;
use image::RgbImage;
use quantette::{ImageRef, Pipeline, deps::palette::Srgb};
use std::collections::HashMap;

pub(super) fn index(image: &RgbImage) -> Result<(Vec<u8>, Vec<u8>)> {
    if let Some(exact) = exact_colors(image) {
        return Ok(exact);
    }
    let pixels: Vec<_> = image
        .pixels()
        .map(|pixel| Srgb::new(pixel[0], pixel[1], pixel[2]))
        .collect();
    let input = ImageRef::new(image.width(), image.height(), &pixels)
        .map_err(|error| anyhow::anyhow!("{error}"))?;
    // Deduplication avoids repeated Oklab conversion without disabling
    // per-pixel Floyd–Steinberg diffusion or reducing the 256-color budget.
    let indexed = Pipeline::new()
        .dedup(true)
        .input_image(input)
        .output_srgb8_indexed_image();
    let colors = indexed
        .palette()
        .iter()
        .flat_map(|color| [color.red, color.green, color.blue])
        .collect();
    Ok((colors, indexed.indices().to_vec()))
}

fn exact_colors(image: &RgbImage) -> Option<(Vec<u8>, Vec<u8>)> {
    let mut lookup = HashMap::with_capacity(256);
    let mut colors = Vec::with_capacity(768);
    let mut indices = Vec::with_capacity(image.len() / 3);
    let mut previous = None;
    for pixel in image.pixels() {
        let index = if let Some((color, index)) = previous
            && color == pixel.0
        {
            index
        } else if let Some(index) = lookup.get(&pixel.0) {
            *index
        } else {
            let index = u8::try_from(colors.len() / 3).ok()?;
            colors.extend_from_slice(&pixel.0);
            lookup.insert(pixel.0, index);
            index
        };
        indices.push(index);
        previous = Some((pixel.0, index));
    }
    (!colors.is_empty()).then_some((colors, indices))
}

#[cfg(test)]
mod tests;
