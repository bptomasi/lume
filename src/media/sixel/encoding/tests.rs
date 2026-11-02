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

mod legacy;

fn encoded(
    pixels: &[u8],
    width: u32,
    height: u32,
    palette: &[u8],
    encoder: Encoder,
) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let (result, _) = profiling::capture_encoder(
        profiling::Quantizer::WuDedup,
        encoder,
        || write(&mut bytes, pixels, width, height, palette),
    );
    result.map_err(|error| anyhow::anyhow!("{error}"))?;
    Ok(bytes)
}

#[test]
fn dense_and_scatter_are_byte_exact_against_the_frozen_writer() -> Result<()> {
    for (index, colors) in [1, 2, 3, 16, 255, 256].into_iter().enumerate() {
        for (offset, width) in
            [1, 2, 3, 255, 256, 511, 10_240].into_iter().enumerate()
        {
            let height = 7 + ((index + offset) % 6) as u32;
            compare_indexed(width, height, colors)?;
        }
    }
    Ok(())
}

fn compare_indexed(width: u32, height: u32, colors: usize) -> Result<()> {
    let palette: Vec<_> =
        (0..colors).flat_map(|n| [n as u8, 37, 211]).collect();
    let mut random = 0x94a4_172fu32;
    let pixels: Vec<_> = (0..width * height)
        .map(|index| {
            random ^= random << 13;
            random ^= random >> 17;
            random ^= random << 5;
            if index % 37 < 5 {
                0
            } else {
                (random as usize % colors) as u8
            }
        })
        .collect();
    let mut expected = Vec::new();
    legacy::write(&mut expected, &pixels, width, height, &palette)
        .map_err(|error| anyhow::anyhow!("{error}"))?;
    for encoder in [Encoder::Dense, Encoder::Sparse, Encoder::Production] {
        let actual = encoded(&pixels, width, height, &palette, encoder)?;
        assert_eq!(actual, expected, "{encoder:?} {width}x{height}/{colors}");
        assert_eq!(decode(&actual, width as usize, height as usize)?, pixels);
    }
    Ok(())
}

#[test]
fn invalid_rasters_fail_before_writing_a_header() {
    for (pixels, width, height, palette) in [
        (&[][..], 0, 1, &[0; 3][..]),
        (&[][..], 1, 0, &[0; 3][..]),
        (&[][..], u32::MAX, u32::MAX, &[0; 3][..]),
        (&[0][..], 1, 2, &[0; 3][..]),
        (&[0][..], 1, 1, &[][..]),
        (&[0][..], 1, 1, &[0; 4][..]),
        (&[0][..], 1, 1, &[0; 771][..]),
        (&[1][..], 1, 1, &[0; 3][..]),
    ] {
        let mut bytes = Vec::new();
        assert!(write(&mut bytes, pixels, width, height, palette).is_err());
        assert!(bytes.is_empty());
    }
}

#[test]
fn cancellation_interrupts_planning_and_emission_without_a_footer() -> Result<()>
{
    for encoder in [Encoder::Dense, Encoder::Sparse] {
        for point in [Checkpoint::Plan, Checkpoint::Emit] {
            let pixels: Vec<_> =
                (0..4096 * 12).map(|n| (n % 16) as u8).collect();
            let mut calls = 0;
            let mut hook = |current| {
                if current == point {
                    calls += 1;
                }
                current == point && calls == 3
            };
            let mut checks = Checks {
                cancel: None,
                hook: Some(&mut hook),
            };
            let raster = validate(&pixels, 4096, 12, &[0; 48], &mut checks)
                .map_err(|error| anyhow::anyhow!("{error}"))?;
            let mut bytes = Vec::new();
            let output =
                Output::new(&mut bytes, 4096, 12, &[0; 48], usize::MAX)
                    .map_err(|error| anyhow::anyhow!("{error}"))?;
            assert!(encode(output, raster, encoder, &mut checks).is_err());
            assert_eq!(calls, 3);
            assert!(!bytes.ends_with(b"\x1b\\"));
        }
    }
    Ok(())
}

#[test]
fn every_six_row_mask_and_run_matches_the_old_plane_writer() -> Result<()> {
    for mask in 0..64u8 {
        for width in [1, 3, 4, 255, 256, 511] {
            let pixels: Vec<_> = (0..6)
                .flat_map(|row| {
                    std::iter::repeat_n(u8::from(mask & (1 << row) == 0), width)
                })
                .collect();
            let mut expected = Vec::new();
            legacy::write(&mut expected, &pixels, width as u32, 6, &[0; 6])
                .map_err(|error| anyhow::anyhow!("{error}"))?;
            for encoder in [Encoder::Dense, Encoder::Sparse] {
                assert_eq!(
                    encoded(&pixels, width as u32, 6, &[0; 6], encoder)?,
                    expected
                );
            }
        }
    }
    Ok(())
}

#[test]
fn exact_size_cap_includes_packets_pending_output_and_footer() -> Result<()> {
    let (width, height) = (1024, 30);
    let pixels: Vec<_> = (0..width * height).map(|n| (n * 17) as u8).collect();
    for encoder in [Encoder::Dense, Encoder::Sparse] {
        let expected = encoded(&pixels, width, height, &[0; 768], encoder)?;
        assert!(expected.len() > 16_384);
        for difference in [-1isize, 0, 1] {
            let mut bytes = Vec::new();
            let mut checks = Checks {
                cancel: None,
                hook: None,
            };
            let raster =
                validate(&pixels, width, height, &[0; 768], &mut checks)
                    .map_err(|error| anyhow::anyhow!("{error}"))?;
            let limit = expected
                .len()
                .checked_add_signed(difference)
                .context("Cap")?;
            let output =
                Output::new(&mut bytes, width, height, &[0; 768], limit)
                    .map_err(|error| anyhow::anyhow!("{error}"))?;
            let result = encode(output, raster, encoder, &mut checks);
            assert_eq!(result.is_ok(), difference >= 0);
            if result.is_ok() {
                assert_eq!(bytes, expected);
            } else {
                assert!(!bytes.ends_with(b"\x1b\\"));
            }
        }
    }
    Ok(())
}

#[test]
fn consecutive_frames_keep_their_own_palettes_and_full_framing() -> Result<()> {
    let mut expected = Vec::new();
    let mut actual = Vec::new();
    for palette in [[17, 34, 51, 68, 85, 102], [255, 0, 0, 0, 255, 0]] {
        let pixels: Vec<_> = (0..78).map(|n| (n % 2) as u8).collect();
        legacy::write(&mut expected, &pixels, 6, 13, &palette)
            .map_err(|error| anyhow::anyhow!("{error}"))?;
        write(&mut actual, &pixels, 6, 13, &palette)
            .map_err(|error| anyhow::anyhow!("{error}"))?;
    }
    assert_eq!(actual, expected);
    Ok(())
}

#[test]
fn profiling_reports_the_planner_chosen_by_production_dispatch() -> Result<()> {
    for (width, calls) in [(1, [1, 0]), (1200, [0, 1])] {
        let pixels: Vec<_> = (0..width * 6).map(|n| n as u8).collect();
        let mut bytes = Vec::new();
        let (result, sample) =
            profiling::capture(profiling::Quantizer::WuDedup, || {
                write(&mut bytes, &pixels, width, 6, &[0; 768])
            });
        result.map_err(|error| anyhow::anyhow!("{error}"))?;
        assert_eq!(sample.encoder_calls, calls);
        assert!(sample.encoder_scratch_bytes > 0);
    }
    Ok(())
}

#[test]
fn sparse_retains_long_runs_and_zero_gaps() -> Result<()> {
    let masks = [0, 1, 3, 63, 21, 42];
    let width = masks.len() * 511;
    let pixels: Vec<_> = (0..6)
        .flat_map(|row| {
            masks.iter().flat_map(move |mask| {
                std::iter::repeat_n(u8::from(mask & (1 << row) == 0), 511)
            })
        })
        .collect();
    let mut expected = Vec::new();
    legacy::write(&mut expected, &pixels, width as u32, 6, &[0; 6])
        .map_err(|error| anyhow::anyhow!("{error}"))?;
    let actual = encoded(&pixels, width as u32, 6, &[0; 6], Encoder::Sparse)?;
    assert_eq!(actual, expected);
    assert_eq!(decode(&actual, width, 6)?, pixels);
    Ok(())
}
