//! Build capabilities change presentation and transport, never stored data.

mod common;

use anyhow::{Context, Result};
use lume::{media, reader};

#[test]
fn formula_projection_and_heading_index_follow_available_capabilities()
-> Result<()> {
    let source = "# $x^2$\n\nBefore $x^2$ after.\n\n$$\\frac{1}{2}$$";
    let options = reader::RenderOptions {
        latex_formulas: true,
        graphical_formulas: true,
    };
    let rendered = reader::render_with_options(source, 80, options);
    let index =
        reader::index::DocumentIndex::parse_with_options(source, options);
    let expected = if cfg!(feature = "math") {
        "x²"
    } else {
        "$x^2$"
    };
    assert_eq!(index.headings[0].title, expected);
    assert!(rendered.text.contains(expected));
    assert_eq!(rendered.images.is_empty(), !cfg!(feature = "graphic-math"));
    if !cfg!(feature = "math") {
        assert!(rendered.text.contains("$$\\frac{1}{2}$$"));
    }
    let offset = rendered.text.find(expected).context("Formula missing")?;
    let range =
        reader::to_source(&rendered.mappings, offset..offset + expected.len());
    assert_eq!(range.map(|range| &source[range]), Some("$x^2$"));
    Ok(())
}

#[test]
fn code_keeps_exact_contents_with_or_without_syntax_colors() {
    let code = "fn main() {\n    let café = 42;\n}\n";
    let source = format!("```rust\n{code}```");
    let rendered = reader::render(&source, 80);
    assert_eq!(rendered.text, format!("{code}\n"));
    let code_rows = rendered
        .lines
        .iter()
        .zip(&rendered.code)
        .filter(|(_, code)| **code)
        .map(|(line, _)| line.to_string())
        .collect::<Vec<_>>();
    assert_eq!(code_rows, code.lines().collect::<Vec<_>>());
    let colored = rendered
        .lines
        .iter()
        .flat_map(|line| &line.spans)
        .any(|span| span.style.fg.is_some());
    assert_eq!(colored, cfg!(feature = "syntax"));
    assert_eq!(
        reader::index::DocumentIndex::parse(&source).code[0].text,
        code
    );
}

#[test]
fn raster_images_use_available_protocols_without_unavailable_work() -> Result<()>
{
    let mut bytes = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
        2,
        2,
        image::Rgb([255, 0, 0]),
    ))
    .write_to(&mut bytes, image::ImageFormat::Png)?;
    let key = media::ImageKey {
        url: "https://example.org/image.png".into(),
        width: 8,
        height: 4,
        background: [0; 3],
        detail: Some(media::ImageDetail {
            zoom: 1,
            pan: [0; 2],
            protocol: media::ImageProtocol::Sixel,
            cell_pixels: (10, 20),
            slice: None,
        }),
    };
    let image = media::Decoder::default().render(bytes.get_ref(), &key)?;
    assert_eq!(image.dimensions, (2, 2));
    assert_eq!(image.native.is_some(), cfg!(feature = "sixel"));
    assert!(image.rows.is_none());
    if !cfg!(feature = "sixel") {
        assert!(image.cells.content.iter().all(|cell| {
            !cell.symbol().contains('\u{1b}')
                && cell.fg == ratatui::style::Color::Rgb(255, 0, 0)
        }));
    }
    Ok(())
}

#[test]
fn help_and_server_configuration_remain_portable() -> Result<()> {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_lume"))
        .arg("--help")
        .output()?;
    assert!(output.status.success());
    assert!(String::from_utf8(output.stdout)?.contains("sync"));
    let source = "[[servers]]\nid = 'home'\nurl = 'https://reader.example'\n\
                  token_env = 'LUME_TOKEN'\n\n[ui]\nlatex_formulas = true\n\
                  latex_graphics = true\n\n[ui.images]\nprotocol = 'sixel'\n";
    let config = lume::config::decode(source)?;
    let encoded = lume::config::encode(&config)?;
    assert_eq!(lume::config::decode(&encoded)?, config);
    let (_directory, store, _) = common::seeded_store()?;
    let mut app = lume::app::App::new(store, false)?;
    common::fixture_command(&mut app, "image-protocol sixel")?;
    assert_eq!(
        app.workspace().images.protocol,
        lume::model::ImagePreference::Sixel
    );
    assert_eq!(app.uses_sixel_images(), cfg!(feature = "sixel"));
    common::fixture_command(&mut app, "latex-formulas graphics")?;
    assert!(app.workspace().latex_formulas && app.workspace().latex_graphics);
    if !cfg!(feature = "math") {
        assert!(app.status().contains("unavailable"));
    }
    app.end_session()?;
    Ok(())
}

#[test]
#[cfg(not(feature = "miniflux"))]
fn disabled_server_collection_reports_capability_without_changing_health()
-> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("library.sqlite3");
    let store = lume::store::Store::open(&path)?;
    store.add_server("https://reader.example", "LUME_TOKEN")?;
    let before = store.servers()?;
    for _ in 0..2 {
        let error = lume::source::collect_once(&path)
            .err()
            .context("Disabled server must explain unavailable collection")?;
        assert!(error.to_string().contains("Miniflux unavailable"));
        assert!(error.to_string().contains("Collected 0 publisher feeds"));
    }
    let after = store.servers()?;
    assert_eq!(after[0].error, before[0].error);
    assert_eq!(after[0].sync, before[0].sync);
    assert!(store.source_health()?.is_empty());
    Ok(())
}
