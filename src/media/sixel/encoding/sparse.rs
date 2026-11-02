//! A1: stable counting scatter visits columns before rows, avoiding K*W scans.
use super::{Checkpoint, Checks, Output, Raster};
use icy_sixel::SixelResult;

struct Band {
    events: Vec<u32>,
    offsets: [usize; 257],
    cursors: [usize; 256],
}

pub(super) fn preferred(raster: Raster<'_>) -> bool {
    // Paired probes and structured antialiasing favor dense below 48 colors.
    // Fixed arrays also penalize small widths. Compare complete planner bounds
    // before allocating either representation; selection never inspects entropy.
    raster.colors >= 48
        && workspace(raster)
            <= raster.colors * raster.width + size_of::<Vec<u8>>()
}

fn workspace(raster: Raster<'_>) -> usize {
    raster.width * raster.height.min(6) * size_of::<u32>() + size_of::<Band>()
}

// Keep fixed sparse workspace off the dense caller's stack, including with LTO.
#[inline(never)]
pub(super) fn write(
    output: &mut Output<'_>,
    raster: Raster<'_>,
    checks: &mut Checks<'_>,
) -> SixelResult<()> {
    let mut plan = Band {
        events: vec![0; raster.width * raster.height.min(6)],
        offsets: [0; 257],
        cursors: [0; 256],
    };
    output
        .scratch(plan.events.capacity() * size_of::<u32>() + size_of::<Band>());
    for (band, pixels) in raster.pixels.chunks(raster.width * 6).enumerate() {
        checks.check(Checkpoint::Plan)?;
        output.band(band);
        plan.scatter(pixels, raster.width, raster.colors, checks)?;
        plan.paint(output, raster.colors, checks)?;
    }
    Ok(())
}

impl Band {
    fn scatter(
        &mut self,
        pixels: &[u8],
        width: usize,
        colors: usize,
        checks: &mut Checks<'_>,
    ) -> SixelResult<()> {
        self.offsets.fill(0);
        for chunk in pixels.chunks(4096) {
            checks.check(Checkpoint::Plan)?;
            for &color in chunk {
                self.offsets[usize::from(color) + 1] += 1;
            }
        }
        for color in 0..colors {
            self.offsets[color + 1] += self.offsets[color];
        }
        self.cursors[..colors].copy_from_slice(&self.offsets[..colors]);
        for column in 0..width {
            if column.is_multiple_of(1024) {
                checks.check(Checkpoint::Plan)?;
            }
            for (row, pixels) in pixels.chunks(width).enumerate() {
                let color = usize::from(pixels[column]);
                let slot = self.cursors[color];
                self.events[slot] = ((column as u32) << 6) | (1 << row);
                self.cursors[color] += 1;
            }
        }
        Ok(())
    }

    fn paint(
        &self,
        output: &mut Output<'_>,
        colors: usize,
        checks: &mut Checks<'_>,
    ) -> SixelResult<()> {
        let mut painted = false;
        for color in 0..colors {
            checks.check(Checkpoint::Emit)?;
            let events =
                &self.events[self.offsets[color]..self.offsets[color + 1]];
            if events.is_empty() {
                continue;
            }
            output.color(color, painted)?;
            emit(output, events, checks)?;
            output.flush()?;
            painted = true;
        }
        Ok(())
    }
}

fn emit(
    output: &mut Output<'_>,
    events: &[u32],
    checks: &mut Checks<'_>,
) -> SixelResult<()> {
    let (mut index, mut next, mut checked) = (0, 0, 0);
    while index < events.len() {
        if index >= checked {
            checks.check(Checkpoint::Emit)?;
            checked = index + 1024;
        }
        let column = (events[index] >> 6) as usize;
        let mut mask = 0;
        while index < events.len() && (events[index] >> 6) as usize == column {
            mask |= (events[index] & 63) as u8;
            index += 1;
        }
        if column > next {
            output.run(0, column - next)?;
        }
        output.run(mask, 1)?;
        next = column + 1;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;

    #[test]
    fn scatter_reuses_one_allocation_and_retains_column_order() -> Result<()> {
        let mut plan = Band {
            events: vec![0; 511 * 6],
            offsets: [0; 257],
            cursors: [0; 256],
        };
        let pointer = plan.events.as_ptr();
        let capacity = plan.events.capacity();
        let mut checks = Checks {
            cancel: None,
            hook: None,
        };
        for rows in 1..=6 {
            let pixels: Vec<_> =
                (0..511 * rows).map(|n| (n * 17) as u8).collect();
            plan.scatter(&pixels, 511, 256, &mut checks)
                .map_err(|error| anyhow::anyhow!("{error}"))?;
            assert_eq!(plan.events.as_ptr(), pointer);
            assert_eq!(plan.events.capacity(), capacity);
            assert_eq!(plan.offsets[256], pixels.len());
            for color in 0..256 {
                let events =
                    &plan.events[plan.offsets[color]..plan.offsets[color + 1]];
                assert!(
                    events.windows(2).all(|pair| pair[0] >> 6 <= pair[1] >> 6)
                );
                assert!(events.iter().all(|event| (event & 63) < 1 << rows));
            }
        }
        Ok(())
    }

    #[test]
    fn production_crossover_never_increases_planner_storage() {
        for colors in [1, 2, 3, 8, 16, 24, 32, 48, 64, 128, 255, 256] {
            for width in [1, 8, 17, 18, 32, 128, 511, 512, 513, 1200, 10240] {
                for height in [1, 5, 6, 20, 60] {
                    let raster = Raster {
                        pixels: &[],
                        width,
                        height,
                        colors,
                    };
                    if preferred(raster) {
                        assert!(colors >= 48);
                        assert!(
                            workspace(raster)
                                <= colors * width + size_of::<Vec<u8>>()
                        );
                    }
                }
            }
        }
        let raster = Raster {
            pixels: &[],
            width: 1200,
            height: 60,
            colors: 256,
        };
        assert!(preferred(raster));
        assert!(!preferred(Raster { width: 1, ..raster }));
        assert!(!preferred(Raster {
            colors: 16,
            ..raster
        }));
        assert!(!preferred(Raster {
            colors: 34,
            ..raster
        }));
        assert!(preferred(Raster {
            colors: 48,
            ..raster
        }));
    }
}
