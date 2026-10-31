//! Profile the real image pipeline; experimental policies never become defaults.
#[path = "sixel_profile/fixtures.rs"]
mod fixtures;
#[path = "sixel_profile/quality.rs"]
mod quality;

use anyhow::{Context, Result, bail, ensure};
use clap::Parser;
use lume::media::{
    Decoder, ImageDetail, ImageKey, ImageProtocol, ImageSlice, Thumbnail,
    profiling::{self, Quantizer},
};
use ratatui::buffer::Buffer;
use ratatui_image::protocol::Protocol;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf};

#[derive(Parser)]
struct Arguments {
    #[arg(long, default_value = "diagram")]
    case: String,
    #[arg(long)]
    input: Option<PathBuf>,
    #[arg(long, default_value = "dedup")]
    strategy: String,
    #[arg(long, default_value_t = 2, value_parser = clap::value_parser!(u8).range(1..=20))]
    repeats: u8,
    #[arg(long, default_value_t = 120)]
    width: u16,
    #[arg(long, default_value_t = 36)]
    height: u16,
    #[arg(long)]
    inline: bool,
    #[arg(long)]
    light: bool,
    #[arg(long, default_value_t = 1)]
    zoom: u8,
    #[arg(long, default_value = "/tmp/lume-sixel-profile")]
    output: PathBuf,
    #[arg(long, requires = "actual")]
    compare: Option<PathBuf>,
    #[arg(long, requires = "compare")]
    actual: Option<PathBuf>,
}

fn main() -> Result<()> {
    let args = Arguments::parse();
    if let Some(reference) = &args.compare {
        let actual = args.actual.as_ref().context("Decoded image path")?;
        println!(
            "{}",
            quality::compare(
                image::open(reference)?.into_rgb8(),
                image::open(actual)?.into_rgb8()
            )?
        );
        return Ok(());
    }
    run(args)
}

fn strategy(name: &str) -> Result<Quantizer> {
    match name {
        "reference" => Ok(Quantizer::Reference),
        "dedup" => Ok(Quantizer::WuDedup),
        "direct" => Ok(Quantizer::WuDirect),
        "undithered" => Ok(Quantizer::WuUndithered),
        "median-cut" => Ok(Quantizer::RgbMedianCut),
        _ => bail!("Use reference|dedup|direct|undithered|median-cut"),
    }
}

fn run(args: Arguments) -> Result<()> {
    let method = strategy(&args.strategy)?;
    let bytes = match &args.input {
        Some(path) => fs::read(path)?,
        None => fixtures::source(&args.case)?,
    };
    let key = key(&args);
    fs::create_dir_all(&args.output)?;
    let mut decoder = Decoder::default();
    let mut samples = Vec::new();
    let mut payloads = Vec::new();
    for pass in 0..args.repeats {
        let (rendered, sample) =
            profiling::capture(method, || decoder.render(&bytes, &key));
        payloads = payload(&rendered?)?;
        let digest = hex_digest(&payloads.concat());
        samples.push(json!({"cache": if pass == 0 {"cold"} else {"warm"}, "timing": sample, "wire_sha256": digest}));
    }
    let reference = profiling::fitted_image(&mut decoder, &bytes, &key)?;
    reference.save(args.output.join("reference.png"))?;
    publish(&args, &bytes, &reference, samples, payloads)
}

fn key(args: &Arguments) -> ImageKey {
    ImageKey {
        url: "https://benchmark.example/sixel".into(),
        width: args.width,
        height: args.height,
        background: if args.light { [245; 3] } else { [23, 23, 26] },
        detail: Some(ImageDetail {
            zoom: args.zoom,
            pan: [500; 2],
            protocol: ImageProtocol::Sixel,
            cell_pixels: (10, 20),
            slice: args.inline.then_some(ImageSlice {
                first_row: 0,
                rows: args.height,
            }),
        }),
    }
}

fn payload(image: &Thumbnail) -> Result<Vec<Vec<u8>>> {
    if let Some(Protocol::Sixel(native)) = &image.native {
        return Ok(vec![native.data.as_bytes().to_vec()]);
    }
    let rows = image.rows.as_ref().context("Sixel pixels missing")?;
    let area = image.cells.area;
    let mut buffer = Buffer::empty(area);
    rows.render(area, 0, &mut buffer);
    (0..area.height)
        .map(|row| {
            let symbol = buffer[(0, row)].symbol();
            let first = symbol
                .find("\x1bP")
                .context("Sixel strip framing missing")?;
            ensure!(symbol.ends_with("\x1b\\"), "Incomplete Sixel strip");
            Ok(symbol.as_bytes()[first..].to_vec())
        })
        .collect()
}

fn publish(
    args: &Arguments,
    source: &[u8],
    reference: &image::RgbImage,
    samples: Vec<serde_json::Value>,
    payloads: Vec<Vec<u8>>,
) -> Result<()> {
    for (index, bytes) in payloads.iter().enumerate() {
        fs::write(args.output.join(format!("strip-{index:03}.six")), bytes)?;
    }
    let case = if args.input.is_some() {
        "local-input"
    } else {
        &args.case
    };
    let report = json!({"case": case, "strategy": args.strategy,
        "inline": args.inline, "zoom": args.zoom, "light": args.light,
        "source_bytes": source.len(), "source_sha256": hex_digest(source),
        "fitted_dimensions": reference.dimensions(),
        "reference_sha256": hex_digest(reference.as_raw()),
        "wire_bytes": payloads.iter().map(Vec::len).sum::<usize>(),
        "strips": payloads.len(), "samples": samples,
        "stages": profiling::Step::ALL.map(profiling::Step::label)});
    fs::write(
        args.output.join("sample.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!("{}", args.output.join("sample.json").display());
    Ok(())
}

fn hex_digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
