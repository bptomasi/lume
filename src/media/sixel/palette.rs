//! Preserve terminal-representable diagram colors before quantizing richer images.
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
    // Keep the common small RGB palette path unchanged. Only richer sources
    // need the second attempt at the terminal's representable precision.
    index_colors(image, |color| color)
        .or_else(|| index_colors(image, terminal_color))
}

fn index_colors(
    image: &RgbImage,
    color_key: impl Fn([u8; 3]) -> [u8; 3],
) -> Option<(Vec<u8>, Vec<u8>)> {
    let mut lookup = HashMap::with_capacity(256);
    let mut colors = Vec::with_capacity(768);
    let mut indices = Vec::with_capacity(image.len() / 3);
    let mut previous = None;
    for pixel in image.pixels() {
        let index = if let Some((color, index)) = previous
            && color == pixel.0
        {
            index
        } else {
            let color = color_key(pixel.0);
            if let Some(index) = lookup.get(&color) {
                *index
            } else {
                let index = u8::try_from(colors.len() / 3).ok()?;
                colors.extend_from_slice(&pixel.0);
                lookup.insert(color, index);
                index
            }
        };
        indices.push(index);
        previous = Some((pixel.0, index));
    }
    (!colors.is_empty()).then_some((colors, indices))
}

fn terminal_color(color: [u8; 3]) -> [u8; 3] {
    // SIXEL encodes RGB channels as integer percentages. This is the same
    // rounding as icy_sixel's palette writer, so merging these colors loses
    // no wire precision and avoids dithering already representable edges.
    color.map(|value| ((u16::from(value) * 100 + 127) / 255) as u8)
}

#[cfg(test)]
mod tests;
