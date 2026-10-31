//! Color error is measured against the actual independently decoded protocol.
use anyhow::{Result, ensure};
use image::RgbImage;
use quantette::{
    color_space::srgb8_to_oklab,
    deps::palette::{Srgb, cast},
};
use serde_json::{Value, json};

pub(super) fn compare(reference: RgbImage, actual: RgbImage) -> Result<Value> {
    ensure!(
        reference.dimensions() == actual.dimensions(),
        "Decoded Sixel size drifted"
    );
    let mut squared = 0.0;
    let mut absolute = 0.0;
    let mut maximum = 0;
    for (a, b) in reference.as_raw().iter().zip(actual.as_raw()) {
        let difference = (f64::from(*a) - f64::from(*b)).abs();
        squared += difference * difference;
        absolute += difference;
        maximum = maximum.max(difference as u32);
    }
    let count = reference.as_raw().len() as f64;
    let mse = squared / count;
    let reference_lab = srgb8_to_oklab(cast::from_component_slice::<Srgb<u8>>(
        reference.as_raw(),
    ));
    let actual_lab =
        srgb8_to_oklab(cast::from_component_slice::<Srgb<u8>>(actual.as_raw()));
    let mut delta = reference_lab
        .iter()
        .zip(actual_lab)
        .map(|(a, b)| {
            f64::from(
                ((a.l - b.l).powi(2)
                    + (a.a - b.a).powi(2)
                    + (a.b - b.b).powi(2))
                .sqrt(),
            ) * 100.0
        })
        .collect::<Vec<_>>();
    let mean = delta.iter().sum::<f64>() / delta.len() as f64;
    delta.sort_unstable_by(f64::total_cmp);
    Ok(json!({"rgb_mse": mse, "rgb_mae": absolute / count,
        "rgb_max_error": maximum, "psnr_db": (mse > 0.0).then(|| 10.0 * (255.0_f64.powi(2) / mse).log10()),
        "oklab_delta_mean_x100": mean,
        "oklab_delta_p95_x100": delta[(delta.len() * 95).div_ceil(100) - 1]}))
}
