//! Bounded six-row planning avoids copying a full palette map for every run.
//! icy_sixel owns palette definitions, raster attributes, RLE and DCS framing.
use icy_sixel::{
    SixelResult,
    output::{sixel_node, sixel_output},
};

pub(super) fn write(
    bytes: &mut Vec<u8>,
    pixels: &[u8],
    width: u32,
    height: u32,
    palette: &[u8],
) -> SixelResult<()> {
    let mut output = sixel_output::new(bytes);
    output.encode_header(width as i32, height as i32)?;
    let colors = palette.len() / 3;
    for color in 0..colors {
        output.output_rgb_palette_definition(palette, color as i32, -1)?;
    }
    let width = width as usize;
    let mut map = vec![0; colors * width];
    for (band, pixels) in pixels.chunks(width * 6).enumerate() {
        if band > 0 {
            output.putc('-');
        }
        for (offset, &color) in pixels.iter().enumerate() {
            map[usize::from(color) * width + offset % width] |=
                1 << (offset / width);
        }
        paint(&mut output, &map, width, colors)?;
        if output.fn_write.len() > super::MAX_ENCODED_BYTES {
            return Err("Native pixels exceed 8 MiB".into());
        }
        map.fill(0);
    }
    output.encode_footer()
}

fn paint(
    output: &mut sixel_output<&mut Vec<u8>>,
    map: &[u8],
    width: usize,
    colors: usize,
) -> SixelResult<()> {
    let mut painted = false;
    for (color, row) in map.chunks_exact(width).enumerate() {
        let Some(end) = row.iter().rposition(|bits| *bits != 0) else {
            continue;
        };
        if painted {
            output.putc('$');
        }
        let node = sixel_node {
            pal: color as i32,
            sx: 0,
            mx: (end + 1) as i32,
            map: row[..=end].to_vec(),
        };
        let mut x = 0;
        output.put_node(&mut x, node, colors as i32, -1)?;
        painted = true;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
