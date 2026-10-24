//! Independently reconstruct indexed pixels from the emitted Sixel stream.
use super::*;
use anyhow::{Context, Result, ensure};

#[test]
fn dense_sparse_and_partial_bands_reconstruct_every_pixel() -> Result<()> {
    let palette: Vec<_> = (0..256).flat_map(|n| [n as u8, 0, 255]).collect();
    let (width, height) = (511, 13);
    for sparse in [false, true] {
        let pixels: Vec<_> = (0..width * height)
            .map(|index| {
                if sparse && index % 37 != 0 {
                    0
                } else {
                    (index * 17) as u8
                }
            })
            .collect();
        let mut bytes = Vec::new();
        write(&mut bytes, &pixels, width, height, &palette)
            .map_err(|error| anyhow::anyhow!("{error}"))?;
        assert!(bytes.starts_with(b"\x1bPq\"1;1;511;13"));
        assert!(bytes.ends_with(b"\x1b\\"));
        assert_eq!(decode(&bytes, width as usize, height as usize)?, pixels);
    }
    Ok(())
}

#[test]
fn a_single_color_retains_long_runs_and_the_partial_last_band() -> Result<()> {
    let mut bytes = Vec::new();
    let pixels = vec![0; 1600 * 7];
    write(&mut bytes, &pixels, 1600, 7, &[17, 34, 51])
        .map_err(|error| anyhow::anyhow!("{error}"))?;
    assert!(bytes.contains(&b'!'));
    assert_eq!(decode(&bytes, 1600, 7)?, pixels);
    Ok(())
}

fn decode(bytes: &[u8], width: usize, height: usize) -> Result<Vec<u8>> {
    let mut input = bytes.strip_prefix(b"\x1bPq").context("DCS header")?;
    let mut pixels = vec![None; width * height];
    let (mut x, mut y, mut color) = (0, 0, 0);
    while let Some((&code, rest)) = input.split_first() {
        input = rest;
        match code {
            b'"' => {
                fields(&mut input)?;
            }
            b'#' => {
                color = number(&mut input)? as u8;
                if input.first() == Some(&b';') {
                    fields(&mut input)?;
                }
            }
            b'$' => x = 0,
            b'-' => {
                x = 0;
                y += 6;
            }
            b'!' | b'?'..=b'~' => {
                let run = pixel_run(code, &mut input)?;
                paint_pixels(&mut pixels, width, y, &mut x, color, run)?;
            }
            27 => break,
            _ => anyhow::bail!("Unexpected Sixel byte {code}"),
        }
    }
    pixels
        .into_iter()
        .map(|pixel| pixel.context("Unpainted pixel"))
        .collect()
}

fn pixel_run(code: u8, input: &mut &[u8]) -> Result<(u8, usize)> {
    if code != b'!' {
        return Ok((code, 1));
    }
    let repeat = number(input)?;
    let (&bits, rest) = input.split_first().context("RLE pixel")?;
    *input = rest;
    Ok((bits, repeat))
}

fn paint_pixels(
    pixels: &mut [Option<u8>],
    width: usize,
    y: usize,
    x: &mut usize,
    color: u8,
    (code, repeat): (u8, usize),
) -> Result<()> {
    ensure!((b'?'..=b'~').contains(&code), "Invalid six-bit pixel");
    for column in *x..*x + repeat {
        ensure!(column < width, "Pixel escaped the raster width");
        for bit in 0..6 {
            if (code - b'?') & (1 << bit) != 0 {
                let pixel = pixels
                    .get_mut((y + bit) * width + column)
                    .context("Pixel escaped the raster height")?;
                ensure!(pixel.is_none(), "Opaque pixels unexpectedly overdraw");
                *pixel = Some(color);
            }
        }
    }
    *x += repeat;
    Ok(())
}

fn number(input: &mut &[u8]) -> Result<usize> {
    let length = input
        .iter()
        .take_while(|byte| byte.is_ascii_digit())
        .count();
    let value = std::str::from_utf8(&input[..length])?.parse()?;
    *input = &input[length..];
    Ok(value)
}

fn fields(input: &mut &[u8]) -> Result<()> {
    while input
        .first()
        .is_some_and(|byte| byte.is_ascii_digit() || *byte == b';')
    {
        if input.first() == Some(&b';') {
            *input = &input[1..];
        }
        number(input)?;
    }
    Ok(())
}

#[test]
fn an_obsolete_crop_stops_before_emitting_its_first_sixel_band() {
    let (revision, cancel) = crate::cancellation::test_token();
    revision.store(1, std::sync::atomic::Ordering::Release);
    let mut bytes = Vec::new();
    assert!(
        write_cancellable(
            &mut bytes,
            &[0; 120],
            10,
            12,
            &[0, 0, 0],
            Some(&cancel)
        )
        .is_err()
    );
    assert!(!bytes.ends_with(b"\x1b\\"), "A partial image was completed");
}
