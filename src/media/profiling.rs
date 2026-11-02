//! Scoped image CPU measurements. Disabled rendering reads no clocks.
//!
//! A capture belongs to the calling thread and restores any enclosing capture.
//! Experimental quantizers apply only within that scope; normal rendering keeps
//! the reviewed deduplicated Oklab/Wu and Floyd–Steinberg policy. No URLs, image
//! bytes, telemetry or unbounded event histories are retained.

use serde::Serialize;
use std::{cell::RefCell, time::Instant};

/// Independently timed, non-overlapping image preparation boundaries.
#[derive(Clone, Copy, Debug)]
pub enum Step {
    /// Decode, orient and inspect raster pixels, or parse a vector tree.
    Decode,
    /// Crop, matte and resize raster pixels, or rasterize vector geometry.
    Resize,
    /// Convert/own RGB pixels and compact terminal-equivalent palette entries.
    Pixels,
    /// Detect images representable without lossy quantization.
    Exact,
    /// Deduplicate and convert input colors into Oklab.
    Colors,
    /// Build the palette and its color lookup map.
    Quantize,
    /// Map pixels, with optional continuous error diffusion.
    Dither,
    /// Write Sixel framing, palette definitions, bitplanes and runs.
    Encode,
    /// Unsplit reference Pipeline cost, including its dithering.
    Pipeline,
}

impl Step {
    /// Stable report ordering, also used for bounded production samples.
    pub const ALL: [Self; 9] = [
        Self::Decode,
        Self::Resize,
        Self::Pixels,
        Self::Exact,
        Self::Colors,
        Self::Quantize,
        Self::Dither,
        Self::Encode,
        Self::Pipeline,
    ];

    /// Machine-readable stage label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Decode => "decode",
            Self::Resize => "resize_matte",
            Self::Pixels => "pixel_buffers",
            Self::Exact => "exact_colors",
            Self::Colors => "color_preparation",
            Self::Quantize => "quantize",
            Self::Dither => "dither_map",
            Self::Encode => "encode",
            Self::Pipeline => "reference_pipeline",
        }
    }
}

/// Developer-only comparisons; no application configuration selects these.
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub enum Quantizer {
    /// Reviewed Wu/Oklab palette, deduplicated colors and Floyd–Steinberg.
    #[default]
    WuDedup,
    /// Wu/Oklab without deduplicating the input pixels.
    WuDirect,
    /// Deduplicated Wu/Oklab without error diffusion.
    WuUndithered,
    /// Existing icy_sixel RGB median-cut and Floyd–Steinberg alternative.
    RgbMedianCut,
    /// Original copying Pipeline implementation, for equivalence and timing.
    Reference,
}

/// Developer-only encoder comparisons, independent from palette preparation.
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub enum Encoder {
    /// Actual production dispatch, with identical pixels and protocol bytes.
    #[default]
    Production,
    /// Borrowed dense planes and aggregated writer runs (A0).
    Dense,
    /// Stable counting scatter and aggregated writer runs (A1).
    Sparse,
}

/// Nanosecond CPU wall samples; total also includes untimed ownership overhead.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Sample {
    /// Scoped encoder selection; production rendering uses its default.
    pub encoder: Encoder,
    /// Peak planner storage plus the writer's pending String capacity.
    pub encoder_scratch_bytes: usize,
    /// Number of frames planned with dense and sparse storage, respectively.
    pub encoder_calls: [u32; 2],
    /// Scoped algorithm selection; never persisted as an application setting.
    pub quantizer: Quantizer,
    /// Whole capture duration, including allocations and uninstrumented work.
    pub total_ns: u64,
    /// Accumulated durations in `Step::ALL` order.
    pub stages_ns: [u64; 9],
    /// Number of measured calls per stage; absent stages remain zero.
    pub calls: [u32; 9],
}

thread_local! {
    static ACTIVE: RefCell<Option<Sample>> = const { RefCell::new(None) };
}

struct Restore(Option<Sample>);

impl Drop for Restore {
    fn drop(&mut self) {
        ACTIVE.with_borrow_mut(|active| *active = self.0.take());
    }
}

/// Measure the actual rendering path with an explicitly scoped quantizer.
pub fn capture<T>(
    quantizer: Quantizer,
    work: impl FnOnce() -> T,
) -> (T, Sample) {
    capture_encoder(quantizer, Encoder::Production, work)
}

/// Measure one scoped encoder variant without changing application settings.
pub fn capture_encoder<T>(
    quantizer: Quantizer,
    encoder: Encoder,
    work: impl FnOnce() -> T,
) -> (T, Sample) {
    let previous = ACTIVE.replace(Some(Sample {
        quantizer,
        encoder,
        ..Sample::default()
    }));
    let _restore = Restore(previous);
    let started = Instant::now();
    let value = work();
    let mut sample = ACTIVE.take().unwrap_or_default();
    sample.total_ns = nanos(started);
    (value, sample)
}

pub(super) fn encoder() -> Encoder {
    ACTIVE.with_borrow(|active| {
        active
            .as_ref()
            .map_or(Encoder::default(), |value| value.encoder)
    })
}

pub(super) fn encoder_scratch(bytes: usize) {
    ACTIVE.with_borrow_mut(|active| {
        if let Some(sample) = active {
            sample.encoder_scratch_bytes =
                sample.encoder_scratch_bytes.max(bytes);
        }
    });
}

pub(super) fn record_encoder(sparse: bool) {
    ACTIVE.with_borrow_mut(|active| {
        if let Some(sample) = active {
            sample.encoder_calls[usize::from(sparse)] += 1;
        }
    });
}

pub(super) fn quantizer() -> Quantizer {
    ACTIVE.with_borrow(|active| {
        active
            .as_ref()
            .map_or(Quantizer::default(), |value| value.quantizer)
    })
}

pub(super) fn measure<T>(step: Step, work: impl FnOnce() -> T) -> T {
    let started =
        ACTIVE.with_borrow(|active| active.is_some().then(Instant::now));
    let value = work();
    if let Some(started) = started {
        ACTIVE.with_borrow_mut(|active| {
            if let Some(sample) = active {
                let index = step as usize;
                sample.stages_ns[index] += nanos(started);
                sample.calls[index] += 1;
            }
        });
    }
    value
}

fn nanos(started: Instant) -> u64 {
    started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
}

/// Obtain the same oriented, matted and fitted reference pixels as rendering.
/// Slice clipping and terminal encoding are deliberately excluded.
///
/// # Errors
/// Returns the normal decoder's byte, dimension, format or geometry errors.
pub fn fitted_image(
    decoder: &mut super::Decoder,
    bytes: &[u8],
    key: &super::ImageKey,
) -> anyhow::Result<image::RgbImage> {
    let (image, _) = decoder.fit(bytes, key)?;
    Ok(image.into_rgb8())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captures_are_thread_local_and_nested_scopes_restore_the_parent() {
        let (value, sample) = capture(Quantizer::WuDirect, || {
            assert!(matches!(quantizer(), Quantizer::WuDirect));
            let (_, child) = capture(Quantizer::RgbMedianCut, || {
                measure(Step::Quantize, || 7)
            });
            assert_eq!(child.calls[Step::Quantize as usize], 1);
            assert!(matches!(quantizer(), Quantizer::WuDirect));
            let thread = std::thread::spawn(|| {
                assert!(matches!(quantizer(), Quantizer::WuDedup));
                assert!(ACTIVE.with_borrow(Option::is_none));
            });
            assert!(thread.join().is_ok());
            measure(Step::Encode, || 42)
        });
        assert_eq!(value, 42);
        assert_eq!(sample.calls[Step::Encode as usize], 1);
        assert_eq!(sample.calls[Step::Quantize as usize], 0);
        assert!(ACTIVE.with_borrow(Option::is_none));
        assert!(matches!(quantizer(), Quantizer::WuDedup));
    }
}
