//! Optional local formula outlines retain source identity and worker bounds.
mod common;
use anyhow::{Context, Result};
use lume::{
    media::{Decoder, ImageDetail, ImageKey, ImageProtocol},
    reader,
};

fn options() -> reader::RenderOptions {
    reader::RenderOptions {
        latex_formulas: true,
        graphical_formulas: true,
    }
}

#[test]
fn styled_arxiv_terms_remain_in_the_paragraph_and_keep_exact_source()
-> Result<()> {
    use ratatui::style::Modifier;
    let source = r"Verse uses $\textit{Chandas}$, rhythmic $\textit{Laghu}$ and $\textit{Guru}$ syllables. We introduce $\textit{ShlokaDhara}$ for $\textit{Text-to-Recite}$ synthesis and $\textit{Laghu-Guru}$ analysis.";
    let expected = "Verse uses Chandas, rhythmic Laghu and Guru syllables. We introduce ShlokaDhara for Text-to-Recite synthesis and Laghu-Guru analysis.";
    for width in [40, 80, 120] {
        let rendered = reader::render_with_options(source, width, options());
        assert_eq!(rendered.text.trim(), expected);
        assert!(rendered.images.is_empty());
        assert!(
            rendered
                .lines
                .iter()
                .flat_map(|line| &line.spans)
                .any(|span| span.content.contains("Chandas")
                    && span.style.add_modifier.contains(Modifier::ITALIC))
        );
        let start =
            rendered.text.find("ShlokaDhara").context("Term missing")?;
        let original = reader::to_source(
            &rendered.mappings,
            start..start + "ShlokaDhara".len(),
        )
        .context("Source missing")?;
        assert_eq!(&source[original], r"$\textit{ShlokaDhara}$");
    }
    Ok(())
}

#[test]
fn long_text_labels_do_not_request_graphics_based_on_their_length() {
    let source = r"A $\textbf{very long descriptive model name}$ stays inline beside $\texttt{encoder-decoder}$ and $\textit{A\&B}$.";
    let rendered = reader::render_with_options(source, 120, options());
    assert!(rendered.images.is_empty());
    assert_eq!(
        rendered.text.trim(),
        "A very long descriptive model name stays inline beside encoder-decoder and A&B."
    );
}

#[test]
fn short_spatial_inline_formulas_get_outlines_and_keep_original_source()
-> Result<()> {
    for latex in [
        r"\boxed{x}",
        r"\cancel{x}",
        r"\underbrace{x}",
        r"\widehat{xy}",
    ] {
        let source = format!("Before ${latex}$ after.");
        let rendered = reader::render_with_options(&source, 80, options());
        let slot = rendered.images.first().context("Missing inline outline")?;
        let formula = slot.formula.as_ref().context("Not local formula")?;
        assert_eq!(&*formula.latex, latex);
        assert_eq!(&source[slot.source.clone()], format!("${latex}$"));
        assert!(formula.svg.windows(5).any(|text| text == b"<path"));
    }
    let inline =
        reader::render_with_options("Before $x+y$ after.", 80, options());
    assert!(inline.images.is_empty());
    Ok(())
}

#[test]
fn vector_formulas_typeset_real_scripts_and_fractions_without_external_fonts()
-> Result<()> {
    for latex in [
        r"\ell_\infty",
        r"O(\log^{3/2} n)",
        r"\frac{a_1+\ell_\infty}{\log^{3/2} n}",
        r"\sum_{i=1}^{n} \frac{x_i^2}{1+x_i}",
    ] {
        let source = format!("Before $${latex}$$ after.");
        let rendered = reader::render_with_options(&source, 80, options());
        let slot = rendered.images.first().context("Missing formula image")?;
        let formula = slot.formula.as_ref().context("Remote formula")?;
        let svg = std::str::from_utf8(&formula.svg)?;
        assert!(svg.contains("<path"));
        assert!(!svg.contains("<text") && !svg.contains("<image"));
        assert!(!svg.contains("href=") && !svg.contains("@font-face"));
        assert_eq!(&*formula.latex, latex);
        let display =
            rendered.text.find(&slot.alt).context("Caption missing")?;
        assert_eq!(
            reader::to_source(
                &rendered.mappings,
                display..display + slot.alt.len()
            ),
            Some(slot.source.clone())
        );
        assert!(
            rendered.text.contains("Before")
                && rendered.text.contains("after.")
        );
        assert_eq!(&source[slot.source.clone()], format!("$${latex}$$"));
    }
    Ok(())
}

#[test]
fn invalid_excessive_or_unsupported_graphics_keep_readable_text() {
    for source in [
        r"$\unknown{x}$".to_owned(),
        format!("${}x{}$", "{".repeat(80), "}".repeat(80)),
    ] {
        let rendered = reader::render_with_options(&source, 80, options());
        assert!(rendered.images.is_empty());
        assert!(rendered.text.contains(&source));
    }
    let source = "$$x^2$$ ".repeat(40);
    let rendered = reader::render_with_options(&source, 80, options());
    assert_eq!(rendered.images.len(), 32);
    assert_eq!(rendered.text.matches("x²").count(), 40);
}

#[test]
fn local_formula_outlines_render_at_native_resolution() -> Result<()> {
    let rendered = reader::render_with_options(
        r"$$\frac{1}{2}+x_\infty^{3/2}$$",
        80,
        options(),
    );
    let formula = rendered
        .images
        .first()
        .and_then(|s| s.formula.as_ref())
        .context("No graphical formula")?;
    let key = ImageKey {
        url: formula.identity.clone(),
        width: 80,
        height: 16,
        background: [23, 23, 26],
        detail: Some(ImageDetail {
            zoom: 1,
            pan: [0, 0],
            protocol: ImageProtocol::Sixel,
            cell_pixels: (10, 20),
            slice: None,
        }),
    };
    let image = Decoder::default().render(&formula.svg, &key)?;
    assert!(image.native.is_some());
    assert!(image.dimensions.0 > 0 && image.dimensions.1 > 0);
    Ok(())
}

fn command(app: &mut lume::app::App, text: &str) -> Result<()> {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    app.key(KeyEvent::new(KeyCode::Char(':'), KeyModifiers::NONE))?;
    app.paste(text)?;
    app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?;
    Ok(())
}

fn graphics_app()
-> Result<(tempfile::TempDir, lume::app::App, lume::config::Session)> {
    graphics_body(r"Before $\frac{1}{2}+\ell_\infty$ after.")
}

fn graphics_body(
    body: &str,
) -> Result<(tempfile::TempDir, lume::app::App, lume::config::Session)> {
    let (directory, mut store, feed) = common::seeded_store()?;
    let mut parsed =
        lume::source::parse(include_bytes!("fixtures/rss.xml"), &feed.url)?;
    parsed.articles[0].body = body.into();
    parsed.articles[0].published = 2_000_000_000;
    store.ingest(&feed, "Formula journal", &parsed.articles)?;
    let session = lume::config::Session::open(
        &store,
        &directory.path().join("config.toml"),
    )?;
    let mut app = lume::app::App::new(store, false)?;
    app.configure_document(session.initial().clone());
    app.focus = lume::app::Focus::Reader;
    app.workspace.fullscreen = true;
    app.configure_image_terminal(ImageProtocol::Sixel, (10, 20));
    app.resize_workspace(ratatui::layout::Rect::new(0, 0, 100, 32));
    app.enable_background_navigation(
        &directory.path().join("library.sqlite3"),
    )?;
    app.enable_images(directory.path().join("images"), true)?;
    Ok((directory, app, session))
}

fn settle_graphics(app: &mut lume::app::App) -> Result<()> {
    let deadline =
        std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        app.poll_reader();
        app.poll_navigation()?;
        app.poll_note_workspace()?;
        app.poll_images();
        if !app.article_preparing()
            && !app.images_pending()
            && app.images.slots.iter().any(|slot| slot.formula.is_some())
            && app.images.slots.iter().all(|slot| {
                app.image_key(slot)
                    .is_some_and(|key| app.images.thumbnail(&key).is_some())
            })
        {
            return Ok(());
        }
        anyhow::ensure!(
            std::time::Instant::now() < deadline,
            "Formula worker timed out"
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

#[test]
fn graphics_are_local_even_when_remote_images_are_paused_and_preferences_persist()
-> Result<()> {
    let (directory, mut app, session) = graphics_app()?;
    let source = app.body.clone();
    command(&mut app, "images off")?;
    command(&mut app, "latex-formulas graphics")?;
    settle_graphics(&mut app)?;
    assert_eq!(app.body, source);
    assert!(!directory.path().join("images").exists());
    app.end_session()?;
    session.flush()?;
    let config =
        lume::config::read_document(&directory.path().join("config.toml"))?;
    assert!(config.ui.latex_graphics && config.ui.latex_formulas);
    assert!(!config.ui.images.enabled);
    Ok(())
}

#[test]
fn graphic_inspection_copies_original_latex_and_restores_the_reader()
-> Result<()> {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let (_directory, mut app, _) = graphics_app()?;
    command(&mut app, "latex-formulas graphics")?;
    settle_graphics(&mut app)?;
    app.reader_cursor = app.images.slots[0].caption;
    app.reader_column = 0;
    let source = app.body.clone();
    command(&mut app, "image")?;
    let copy =
        app.key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE))?;
    assert_eq!(
        copy,
        lume::app::Action::Copy(r"\frac{1}{2}+\ell_\infty".into())
    );
    app.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))?;
    assert!(matches!(app.mode, lume::app::Mode::Normal));
    assert_eq!(app.body, source);
    app.end_session()?;
    Ok(())
}

#[test]
fn formula_graphics_do_not_escape_table_cells_or_consume_publisher_image_slots()
{
    let table = "| Name | Value |\n| --- | --- |\n| Power | $x^2$ |\n| Ratio | $\\frac{1}{2}$ |";
    let rendered = reader::render_with_options(table, 80, options());
    assert!(rendered.images.is_empty());
    assert!(rendered.text.contains("Power") && rendered.text.contains("x²"));
    assert!(rendered.text.contains("Ratio") && rendered.text.contains("½"));
    assert_eq!(rendered.table_rows.len(), 1);
    let mixed = format!(
        "{}\n\n![Plot](https://example.org/plot.png)",
        "$$x^2$$ ".repeat(40)
    );
    let rendered = reader::render_with_options(&mixed, 80, options());
    assert_eq!(rendered.images.len(), 33);
    assert!(
        rendered
            .images
            .last()
            .is_some_and(|slot| slot.formula.is_none())
    );
}

#[test]
fn system_fallback_fonts_stay_in_text_instead_of_triggering_font_discovery() {
    for source in [r"$\text{你好}$", r"$\text{🧬}$"] {
        let rendered = reader::render_with_options(source, 80, options());
        assert!(rendered.images.is_empty());
        assert!(!rendered.text.trim().is_empty());
    }
}

#[test]
fn arxiv_inline_variables_and_commands_keep_the_paragraph_compact() {
    let source = include_str!("fixtures/arxiv-tool-access.md");
    let rendered = reader::render_with_options(source, 80, options());
    assert!(rendered.text.contains("with B bits of internal state"));
    assert!(rendered.text.contains("memory set M can be internalized"));
    assert!(
        rendered.text.contains("𝚛𝚎𝚊𝚍/𝚠𝚛𝚒𝚝𝚎/𝚖𝚘𝚟𝚎"),
        "{}",
        rendered.text
    );
    assert!(rendered.text.contains("state set Q and tape alphabet Γ"));
    assert!(rendered.text.contains("EQₙ requires 2ⁿ states"));
    assert!(rendered.images.is_empty());
    assert!(rendered.images.iter().all(|slot| slot.rows.len() <= 2));
    assert!(rendered.lines.len() < 24);
}

#[test]
fn dense_formula_pages_remain_native_past_four_slots_without_cache_churn()
-> Result<()> {
    let source = (0..32)
        .map(|n| format!("$$x_{{{n}}}$$ "))
        .collect::<String>();
    let (_directory, mut app, _) = graphics_body(&source)?;
    app.resize_workspace(ratatui::layout::Rect::new(0, 0, 100, 160));
    command(&mut app, "latex-formulas graphics")?;
    settle_graphics(&mut app)?;
    assert_eq!(app.images.slots.len(), 32);
    for slot in &app.images.slots {
        let key = app.image_key(slot).context("Missing formula key")?;
        assert_eq!(
            key.detail.as_ref().map(|d| d.protocol),
            Some(ImageProtocol::Sixel)
        );
        assert!(
            app.images
                .thumbnail(&key)
                .is_some_and(|image| image.rows.is_some())
        );
    }
    for _ in 0..40 {
        assert!(
            !app.poll_images(),
            "Ready formulas must not be admitted again"
        );
    }
    assert!(!app.images_pending());
    app.end_session()?;
    Ok(())
}

#[test]
fn navigation_after_a_formula_policy_change_replaces_the_pending_source_anchor()
-> Result<()> {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let source = format!(
        "{}\n\nA spatial term $\\frac{{a}}{{b}}$.\n\n{}",
        include_str!("fixtures/arxiv-tool-access.md"),
        "A later passage remains available.\n\n".repeat(80)
    );
    let (_directory, mut app, _) = graphics_body(&source)?;
    app.key(KeyEvent::new(KeyCode::Char('G'), KeyModifiers::NONE))?;
    assert!(app.scroll > 30);
    command(&mut app, "latex-formulas graphics")?;
    for code in [KeyCode::Char('g'), KeyCode::Char('g')] {
        app.key(KeyEvent::new(code, KeyModifiers::NONE))?;
    }
    settle_graphics(&mut app)?;
    assert_eq!(app.reader_cursor, 0);
    assert_eq!(app.scroll, 0);
    assert_eq!(app.body, source);
    app.end_session()?;
    Ok(())
}

#[test]
fn nested_lists_keep_numbers_bullets_continuations_and_formula_source_ranges()
-> Result<()> {
    let source = concat!(
        "3. $\\sqrt{x}$\n4. Parent\n   - $\\frac{1}{2}$\n",
        "   - Text before $\\ell_\\infty$ and after.\n5. End"
    );
    let rendered = reader::render_with_options(source, 80, options());
    assert_eq!(rendered.images.len(), 3);
    let lines = rendered
        .lines
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    assert!(lines.iter().any(|line| line.trim() == "3."));
    assert!(lines.iter().any(|line| line == "4. Parent"));
    assert!(lines.iter().any(|line| line == "   •"));
    assert!(lines.iter().any(|line| line == "   • Text before"));
    assert!(lines.iter().any(|line| line.trim() == "and after."));
    assert!(lines.iter().any(|line| line == "5. End"));
    for slot in &rendered.images {
        assert_eq!(
            &source[slot.source.clone()],
            format!(
                "${}$",
                slot.formula.as_ref().context("Formula missing")?.latex
            )
        );
    }
    Ok(())
}
