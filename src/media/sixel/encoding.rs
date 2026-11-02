//! Exact six-row planning; icy_sixel retains protocol and repeat ownership.
use icy_sixel::SixelResult;

use crate::{
    cancellation::Cancellation,
    media::profiling::{self, Encoder},
};

mod dense;
mod output;
mod sparse;

use output::Output;

#[derive(Clone, Copy)]
struct Raster<'a> {
    pixels: &'a [u8],
    width: usize,
    height: usize,
    colors: usize,
}

#[derive(Clone, Copy, PartialEq)]
enum Checkpoint {
    Validate,
    Plan,
    Emit,
}

struct Checks<'a> {
    cancel: Option<&'a Cancellation>,
    #[cfg(test)]
    hook: Option<&'a mut dyn FnMut(Checkpoint) -> bool>,
}

impl Checks<'_> {
    fn check(&mut self, point: Checkpoint) -> SixelResult<()> {
        #[cfg(test)]
        if self.hook.as_mut().is_some_and(|hook| hook(point)) {
            return Err("Image crop superseded".into());
        }
        let _ = point;
        if self.cancel.is_some_and(Cancellation::is_cancelled) {
            return Err("Image crop superseded".into());
        }
        Ok(())
    }
}

#[cfg(test)]
pub(super) fn write(
    bytes: &mut Vec<u8>,
    pixels: &[u8],
    width: u32,
    height: u32,
    palette: &[u8],
) -> SixelResult<()> {
    write_cancellable(bytes, pixels, width, height, palette, None)
}

pub(super) fn write_cancellable(
    bytes: &mut Vec<u8>,
    pixels: &[u8],
    width: u32,
    height: u32,
    palette: &[u8],
    cancel: Option<&Cancellation>,
) -> SixelResult<()> {
    let mut checks = Checks {
        cancel,
        #[cfg(test)]
        hook: None,
    };
    let raster = validate(pixels, width, height, palette, &mut checks)?;
    let output =
        Output::new(bytes, width, height, palette, super::MAX_ENCODED_BYTES)?;
    encode(output, raster, profiling::encoder(), &mut checks)
}

fn validate<'a>(
    pixels: &'a [u8],
    width: u32,
    height: u32,
    palette: &[u8],
    checks: &mut Checks<'_>,
) -> SixelResult<Raster<'a>> {
    checks.check(Checkpoint::Validate)?;
    let colors = dimensions(width, height, pixels.len(), palette)?;
    for chunk in pixels.chunks(4096) {
        checks.check(Checkpoint::Validate)?;
        if chunk.iter().any(|&color| usize::from(color) >= colors) {
            return Err("Sixel index exceeds its palette".into());
        }
    }
    Ok(Raster {
        pixels,
        width: width as usize,
        height: height as usize,
        colors,
    })
}

// These application bounds also prove that W*6, K*W, prefix sums and
// (column << 6) fit usize/u32, including on supported 32-bit platforms.
fn dimensions(
    width: u32,
    height: u32,
    pixels: usize,
    palette: &[u8],
) -> SixelResult<usize> {
    let length = u64::from(width).checked_mul(u64::from(height));
    if width == 0
        || height == 0
        || width > 10_240
        || height > 6_144
        || length.is_none_or(|length| {
            length > 4_000_000 || usize::try_from(length).ok() != Some(pixels)
        })
        || palette.is_empty()
        || palette.len() > 768
        || !palette.len().is_multiple_of(3)
    {
        return Err("Invalid Sixel raster or palette".into());
    }
    Ok(palette.len() / 3)
}

fn encode(
    mut output: Output<'_>,
    raster: Raster<'_>,
    encoder: Encoder,
    checks: &mut Checks<'_>,
) -> SixelResult<()> {
    let sparse = match encoder {
        Encoder::Production => sparse::preferred(raster),
        Encoder::Dense => false,
        Encoder::Sparse => true,
    };
    profiling::record_encoder(sparse);
    if sparse {
        sparse::write(&mut output, raster, checks)?;
    } else {
        dense::write(&mut output, raster, checks)?;
    }
    checks.check(Checkpoint::Emit)?;
    output.finish()
}

#[cfg(test)]
mod tests;
