//! Deterministic, separately named diagram, photographic and synthetic inputs.
use anyhow::{Context, Result, bail};
use image::{DynamicImage, ImageFormat};
use std::io::Cursor;

#[path = "generated.rs"]
mod generated;
#[path = "synthetic.rs"]
mod synthetic;

pub(super) fn source(name: &str) -> Result<Vec<u8>> {
    match name {
        "diagram" => Ok(include_bytes!(
            "../../tests/fixtures/images/request-path.svg"
        )
        .to_vec()),
        "photo" => {
            Ok(include_bytes!("../../tests/fixtures/images/astronaut.png")
                .to_vec())
        }
        "camera" | "chelsea" | "coffee" => natural(name),
        "ui" | "dense" => Ok(synthetic::document(name).into_bytes()),
        "rgba16" => png(synthetic::rgba16()),
        "gradient" | "entropy" | "transparent" | "antialias" => {
            generated::source(name)
        }
        _ => bail!("Unknown fixture: {name}"),
    }
}

fn png(image: DynamicImage) -> Result<Vec<u8>> {
    let mut output = Cursor::new(Vec::new());
    image.write_to(&mut output, ImageFormat::Png)?;
    Ok(output.into_inner())
}

fn natural(name: &str) -> Result<Vec<u8>> {
    // Developer corpus files stay outside the executable's resident image.
    // The paired driver verifies their pinned original hashes before export.
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/images")
        .join(format!("{name}.png"));
    std::fs::read(&path).with_context(|| {
        format!("Read required Sixel fixture {}", path.display())
    })
}
