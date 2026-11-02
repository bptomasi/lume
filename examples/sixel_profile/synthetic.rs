//! Repository-authored structure and true sixteen-bit transparency fixtures.
use image::{DynamicImage, ImageBuffer, Rgba};
use std::fmt::Write;

pub(super) fn document(name: &str) -> String {
    let mut svg = String::from(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="600"><rect width="1200" height="600" fill="#fafafa"/>"##,
    );
    for row in 0..12 {
        for column in 0..8 {
            let x = column * 150;
            let y = row * 50;
            let label = if name == "dense" { "node" } else { "read" };
            // Writing to String is infallible; keep fixture construction simple.
            let _ = write!(
                svg,
                r##"<rect x="{}" y="{}" width="138" height="38" rx="3" fill="#dce8f0" stroke="#244562"/><path d="M{} {}h12" stroke="#cc5030"/><text x="{}" y="{}" font-size="12" fill="#172a38">{label} {row}:{column} / λ</text>"##,
                x + 3,
                y + 3,
                x + 141,
                y + 22,
                x + 9,
                y + 26,
            );
            if name == "ui" {
                let _ = write!(
                    svg,
                    r##"<path d="M{} {}h90m-90 4h60" stroke="#89969f" stroke-width="1"/>"##,
                    x + 12,
                    y + 33,
                );
            }
        }
    }
    svg.push_str("</svg>");
    svg
}

pub(super) fn rgba16() -> DynamicImage {
    let pixels = ImageBuffer::from_fn(1201, 601, |x, y| {
        let alpha = if x < 100 { 0 } else { (y * 65535 / 600) as u16 };
        Rgba([
            (x * 65535 / 1200) as u16,
            ((x * 37 + y * 101) % 65536) as u16,
            50001,
            alpha,
        ])
    });
    DynamicImage::ImageRgba16(pixels)
}
