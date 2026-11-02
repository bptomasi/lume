//! A0: borrow each dense plane and submit equal-mask runs without node copies.
use super::{Checkpoint, Checks, Output, Raster};
use icy_sixel::SixelResult;

pub(super) fn write(
    output: &mut Output<'_>,
    raster: Raster<'_>,
    checks: &mut Checks<'_>,
) -> SixelResult<()> {
    let mut map = vec![0; raster.colors * raster.width];
    output.scratch(map.capacity() + size_of::<Vec<u8>>());
    for (band, pixels) in raster.pixels.chunks(raster.width * 6).enumerate() {
        checks.check(Checkpoint::Plan)?;
        output.band(band);
        plan(&mut map, pixels, raster.width, checks)?;
        paint(output, &map, raster.width, checks)?;
        map.fill(0);
    }
    Ok(())
}

fn plan(
    map: &mut [u8],
    pixels: &[u8],
    width: usize,
    checks: &mut Checks<'_>,
) -> SixelResult<()> {
    for (row, pixels) in pixels.chunks(width).enumerate() {
        for (chunk, pixels) in pixels.chunks(1024).enumerate() {
            checks.check(Checkpoint::Plan)?;
            for (column, &color) in pixels.iter().enumerate() {
                map[usize::from(color) * width + chunk * 1024 + column] |=
                    1 << row;
            }
        }
    }
    Ok(())
}

fn paint(
    output: &mut Output<'_>,
    map: &[u8],
    width: usize,
    checks: &mut Checks<'_>,
) -> SixelResult<()> {
    let mut painted = false;
    for (color, row) in map.chunks_exact(width).enumerate() {
        checks.check(Checkpoint::Emit)?;
        let Some(end) = row.iter().rposition(|&mask| mask != 0) else {
            continue;
        };
        output.color(color, painted)?;
        runs(output, &row[..=end], checks)?;
        output.flush()?;
        painted = true;
    }
    Ok(())
}

fn runs(
    output: &mut Output<'_>,
    row: &[u8],
    checks: &mut Checks<'_>,
) -> SixelResult<()> {
    let mut start = 0;
    let mut checked = 0;
    while start < row.len() {
        if start >= checked {
            checks.check(Checkpoint::Emit)?;
            checked = start + 1024;
        }
        let mask = row[start];
        let length = row[start..]
            .iter()
            .position(|&next| next != mask)
            .unwrap_or(row.len() - start);
        output.run(mask, length)?;
        start += length;
    }
    Ok(())
}
