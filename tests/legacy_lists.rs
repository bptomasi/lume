//! Previously normalized list caches remain readable with original byte anchors.

mod common;

use anyhow::{Context, Result};
use lume::reader;

const LEGACY: &str = concat!(
    "Before opening the detail page:\n\n\n-\n\nLocalQueue\n\n\n-\n\n",
    "priority and priority class\n\n\n-\n\nactive state\n\n\n-\n\n",
    "status such as `Admitted` or `Finished`\n\n",
    "If there is no matching Workload, check the selected namespace."
);

#[test]
fn old_cached_markers_stay_beside_their_text_without_a_refetch() {
    let rendered = reader::render(LEGACY, 80);
    let visible = rendered
        .lines
        .iter()
        .map(ToString::to_string)
        .filter(|row| !row.is_empty())
        .collect::<Vec<_>>();
    assert_eq!(
        visible,
        [
            "Before opening the detail page:",
            "• LocalQueue",
            "• priority and priority class",
            "• active state",
            "• status such as Admitted or Finished",
            "If there is no matching Workload, check the selected namespace.",
        ]
    );
}

#[test]
fn cached_text_and_inline_code_keep_their_original_source_offsets() -> Result<()>
{
    let rendered = reader::render(LEGACY, 24);
    for word in ["LocalQueue", "priority", "Admitted", "namespace"] {
        let source = LEGACY.find(word).context("Source word missing")?;
        let visible =
            rendered.text.find(word).context("Visible word missing")?;
        let projected = reader::to_source(
            &rendered.mappings,
            visible..visible + word.len(),
        )
        .context("Source projection missing")?;
        assert_eq!(projected, source..source + word.len());
        let display = reader::to_display(&rendered.mappings, projected);
        let expected = visible..visible + word.len();
        assert_eq!(display.len(), 1);
        assert_eq!(display.first(), Some(&expected));
    }
    assert_eq!(rendered.text, reader::render(LEGACY, 80).text);
    assert!(rendered.insets.contains(&2));
    Ok(())
}

#[test]
fn intentional_empty_items_and_code_samples_keep_commonmark_semantics() {
    for source in [
        "-\n\nA separate paragraph.",
        "-\n\nA paragraph.\n\n-\n\nAnother paragraph.",
        "```text\n\n\n-\n\nFirst\n\n\n-\n\nSecond\n```",
    ] {
        let rendered = reader::render(source, 80);
        assert!(!rendered.text.contains("• A separate"));
        assert!(!rendered.text.contains("• A paragraph"));
        assert!(!rendered.text.contains("• First"));
    }
}

#[test]
fn code_and_unicode_after_multiple_legacy_runs_keep_exact_source() -> Result<()>
{
    let source = format!(
        "{LEGACY}\n\n~~~text\n\n\n-\n\nliteral\n\n\n-\n\ncode\n~~~\n\n\
         {LEGACY}\n\n**café 界** and [reference](https://example.org)"
    );
    let rendered = reader::render(&source, 80);
    assert_eq!(rendered.text.matches("• LocalQueue").count(), 2);
    assert!(!rendered.text.contains("• literal"));
    assert!(!rendered.text.contains("• code"));
    let visible = rendered.text.find("café 界").context("Unicode missing")?;
    let original = source.find("café 界").context("Source missing")?;
    let range = reader::to_source(&rendered.mappings, visible..visible + 9)
        .context("Unicode source missing")?;
    assert_eq!(range, original..original + 9);
    assert_eq!(
        rendered.links,
        [("reference".into(), "https://example.org/".into())]
    );
    Ok(())
}

#[test]
fn offline_reader_draws_the_cached_list_and_keeps_the_saved_document()
-> Result<()> {
    use lume::{app::App, model::Pane, ui};
    use ratatui::{Terminal, backend::TestBackend, layout::Rect};

    let (_directory, mut store, feed) = common::seeded_store()?;
    let mut parsed =
        lume::source::parse(include_bytes!("fixtures/rss.xml"), &feed.url)?;
    parsed.articles[0].body = LEGACY.into();
    parsed.articles[0].published = 2_000_000_000;
    store.ingest(&feed, "Cached publisher", &parsed.articles)?;
    let mut app = App::new(store, false)?;
    app.focus = Pane::Reader;
    app.workspace.fullscreen = true;
    let screen = Rect::new(0, 0, 120, 40);
    app.resize_workspace(screen);
    let content = ui::workspace_reader_content(screen, &app.workspace);
    let mut terminal = Terminal::new(TestBackend::new(120, 40))?;

    terminal.draw(|frame| ui::draw(frame, &app))?;

    let rows = (content.y..content.bottom())
        .map(|y| {
            (content.x..content.right())
                .map(|x| terminal.backend().buffer()[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>();
    assert!(rows.iter().any(|row| row.starts_with("• LocalQueue")));
    assert!(
        rows.iter()
            .any(|row| row.starts_with("• priority and priority class"))
    );
    assert!(!rows.iter().any(|row| row.trim() == "•"));
    assert_eq!(app.body, LEGACY);
    Ok(())
}
