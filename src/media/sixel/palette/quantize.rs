//! Compose public Quantette stages; keep experiments scoped to measurements.
use crate::media::profiling::{self, Quantizer, Step};
use anyhow::{Context, Result};
use image::RgbImage;
use quantette::{
    ImageRef, IndexedImage, PaletteSize, Pipeline,
    color_space::{oklab_to_srgb8, srgb8_to_oklab},
    dedup,
    deps::palette::{Oklab, Srgb, cast},
    dither::FloydSteinberg,
    wu::{BinnerF32x3, WuF32x3},
};

pub(super) fn run(
    image: &RgbImage,
    method: Quantizer,
) -> Result<(Vec<u8>, Vec<u8>)> {
    if matches!(method, Quantizer::Reference) {
        return reference(image);
    }
    if matches!(method, Quantizer::RgbMedianCut) {
        return median_cut(image);
    }
    // Palette's checked cast borrows the RGB bytes; our code uses no unsafe.
    let pixels = cast::from_component_slice::<Srgb<u8>>(image.as_raw());
    let input = ImageRef::new(image.width(), image.height(), pixels)
        .map_err(|error| anyhow::anyhow!("{error}"))?;
    let indexed = if matches!(method, Quantizer::WuDirect) {
        direct(input)?
    } else {
        deduplicated(input, !matches!(method, Quantizer::WuUndithered))?
    };
    Ok(profiling::measure(Step::Pixels, || {
        let (palette, indices) = indexed.into_parts();
        let colors = oklab_to_srgb8(&palette);
        (cast::into_component_vec(colors), indices)
    }))
}

fn deduplicated(
    input: ImageRef<'_, Srgb<u8>>,
    diffuse: bool,
) -> Result<IndexedImage<Oklab>> {
    let image = profiling::measure(Step::Colors, || {
        dedup::dedup_image_u8_3_counts(input)
            .map(|palette| srgb8_to_oklab(&palette))
    });
    let map = profiling::measure(Step::Quantize, || {
        WuF32x3::run_indexed_image_counts(
            &image,
            BinnerF32x3::oklab_from_srgb8(),
        )
        .map(|wu| wu.color_map(PaletteSize::MAX))
        .context("Empty image has no quantization palette")
    })?;
    Ok(profiling::measure(Step::Dither, || {
        if diffuse {
            FloydSteinberg::new().dither_indexed_to_indexed(image.as_ref(), map)
        } else {
            image.as_ref().map_to_indexed(map)
        }
    }))
}

fn direct(input: ImageRef<'_, Srgb<u8>>) -> Result<IndexedImage<Oklab>> {
    let image =
        profiling::measure(Step::Colors, || input.map_ref(srgb8_to_oklab));
    let map = profiling::measure(Step::Quantize, || {
        WuF32x3::run_image(image.as_ref(), BinnerF32x3::oklab_from_srgb8())
            .map(|wu| wu.color_map(PaletteSize::MAX))
            .context("Empty image has no quantization palette")
    })?;
    Ok(profiling::measure(Step::Dither, || {
        FloydSteinberg::new().dither_image_to_indexed(image.as_ref(), map)
    }))
}

fn reference(image: &RgbImage) -> Result<(Vec<u8>, Vec<u8>)> {
    let pixels: Vec<_> = image
        .pixels()
        .map(|pixel| Srgb::new(pixel[0], pixel[1], pixel[2]))
        .collect();
    let input = ImageRef::new(image.width(), image.height(), &pixels)
        .map_err(|error| anyhow::anyhow!("{error}"))?;
    let indexed = profiling::measure(Step::Pipeline, || {
        Pipeline::new()
            .dedup(true)
            .input_image(input)
            .output_srgb8_indexed_image()
    });
    let colors = indexed
        .palette()
        .iter()
        .flat_map(|color| [color.red, color.green, color.blue])
        .collect();
    Ok((colors, indexed.indices().to_vec()))
}

fn median_cut(image: &RgbImage) -> Result<(Vec<u8>, Vec<u8>)> {
    use icy_sixel::{
        DiffusionMethod, MethodForLargest, MethodForRep, PixelFormat, Quality,
    };
    let mut dither = icy_sixel::dither::sixel_dither::new(256)
        .map_err(|error| anyhow::anyhow!("{error}"))?;
    profiling::measure(Step::Quantize, || {
        dither.initialize(
            image.as_raw(),
            image.width() as i32,
            image.height() as i32,
            PixelFormat::RGB888,
            MethodForLargest::Norm,
            MethodForRep::Pixels,
            Quality::HIGH,
        )
    })
    .map_err(|error| anyhow::anyhow!("{error}"))?;
    dither.set_diffusion_type(DiffusionMethod::FS);
    let indices = profiling::measure(Step::Dither, || {
        dither.apply_palette(
            image.as_raw(),
            image.width() as i32,
            image.height() as i32,
        )
    })
    .map_err(|error| anyhow::anyhow!("{error}"))?;
    Ok((dither.get_palette().to_vec(), indices))
}
