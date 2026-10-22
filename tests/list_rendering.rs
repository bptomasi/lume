//! Publisher lists retain their hierarchy, markers and exact source bytes.

mod common;

use anyhow::{Context, Result};
use lume::{content, reader};

fn rows(source: &str) -> Vec<String> {
    reader::render(source, 80)
        .lines
        .iter()
        .map(ToString::to_string)
        .collect()
}

#[test]
fn loose_markdown_keeps_markers_beside_their_paragraphs() {
    assert_eq!(
        rows("- covered resources\n\n- current reservations and usage"),
        [
            "• covered resources",
            "",
            "• current reservations and usage",
            "",
        ]
    );
}

#[test]
fn html_paragraphs_inside_items_do_not_escape_the_list() {
    let source = content::article_text(
        "<ul><li><p>covered resources</p></li>\
         <li><p>current reservations and usage</p></li></ul>",
        true,
    );
    assert_eq!(
        rows(&source),
        ["• covered resources", "• current reservations and usage"]
    );
}

#[test]
fn ordered_and_nested_html_preserve_numbers_and_hierarchy() {
    let source = content::article_text(
        "<ol start='7'><li><p>Inspect queues</p>\
         <ul><li>Check reservations</li></ul></li>\
         <li>Inspect usage</li><li>Inspect limits</li></ol>",
        true,
    );
    let visible = rows(&source)
        .into_iter()
        .filter(|row| !row.is_empty())
        .collect::<Vec<_>>();
    assert_eq!(
        visible,
        [
            "7. Inspect queues",
            "   • Check reservations",
            "8. Inspect usage",
            "9. Inspect limits",
        ]
    );
}

#[test]
fn list_links_and_emphasis_keep_exact_source_correspondence() -> Result<()> {
    let source = "- **first**\n\n- [café](https://example.org) and `界`";
    let rendered = reader::render(source, 20);
    assert!(rendered.text.contains("• café and 界"));
    for word in ["first", "café", "界"] {
        let start = rendered.text.find(word).context("Visible word missing")?;
        let range =
            reader::to_source(&rendered.mappings, start..start + word.len())
                .context("Source word missing")?;
        assert_eq!(&source[range], word);
    }
    assert_eq!(
        rendered.links,
        [("café".into(), "https://example.org/".into())]
    );
    Ok(())
}

#[test]
fn wrapped_items_use_visual_padding_without_changing_copy_or_source() {
    let source = "- alpha bravo charlie delta echo foxtrot golf";
    let narrow = reader::render(source, 18);
    assert_eq!(
        narrow
            .lines
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        ["• alpha bravo", "charlie delta", "echo foxtrot", "golf"]
    );
    assert_eq!(narrow.insets, [0, 2, 2, 2]);
    assert_eq!(
        narrow.text,
        "• alpha bravo charlie delta echo foxtrot golf\n"
    );
    assert_eq!(narrow.text, reader::render(source, 80).text);
    for (line, range) in narrow.lines.iter().zip(&narrow.text_ranges) {
        assert_eq!(line.to_string(), narrow.text[range.clone()]);
    }
}

fn reading(source: &str) -> Result<(tempfile::TempDir, lume::app::App)> {
    let (directory, mut store, feed) = common::seeded_store()?;
    let mut parsed =
        lume::source::parse(include_bytes!("fixtures/rss.xml"), &feed.url)?;
    parsed.articles[0].body = source.to_owned();
    parsed.articles[0].published = 2_000_000_000;
    store.ingest(&feed, "List journal", &parsed.articles)?;
    let mut app = lume::app::App::new(store, false)?;
    app.focus = lume::model::Pane::Reader;
    app.workspace.fullscreen = true;
    app.workspace.fullscreen_width = 18;
    app.resize_workspace(ratatui::layout::Rect::new(0, 0, 100, 30));
    Ok((directory, app))
}

#[test]
fn continuation_padding_keeps_mouse_cursor_selection_and_reflow_exact()
-> Result<()> {
    use crossterm::event::{
        KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent,
        MouseEventKind,
    };
    use ratatui::{Terminal, backend::TestBackend, layout::Rect};

    let source = "- alpha bravo charlie delta echo foxtrot golf";
    let (_directory, mut app) = reading(source)?;
    let screen = Rect::new(0, 0, 100, 30);
    let content = lume::ui::workspace_reader_content(screen, &app.workspace);
    let mut terminal = Terminal::new(TestBackend::new(100, 30))?;
    terminal.draw(|frame| lume::ui::draw(frame, &app))?;
    let row = (content.x..content.right())
        .map(|x| terminal.backend().buffer()[(x, content.y + 1)].symbol())
        .collect::<String>();
    assert!(row.starts_with("  charlie delta"));
    for kind in [
        MouseEventKind::Down(MouseButton::Left),
        MouseEventKind::Up(MouseButton::Left),
    ] {
        app.mouse(MouseEvent {
            kind,
            column: content.x + 2,
            row: content.y + 1,
            modifiers: KeyModifiers::NONE,
        })?;
    }
    assert_eq!(app.reader_column, 0);
    for character in "v6l".chars() {
        app.key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE))?;
    }
    let copy =
        app.key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE))?;
    assert_eq!(copy, lume::app::Action::Copy("charlie".into()));
    app.resize_reader(40, 20);
    let copy =
        app.key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE))?;
    assert_eq!(copy, lume::app::Action::Copy("charlie".into()));
    assert_eq!(app.body, source);
    Ok(())
}

#[test]
fn multi_paragraph_items_keep_code_and_following_items_in_their_owner() {
    let source = content::article_text(
        "<ul><li><p>Inspect queues</p><p>Then check reservations.</p>\
         <pre><code class='language-rust'>let capacity = 42;</code></pre>\
         </li><li>Inspect usage</li></ul><p>Outside the list.</p>",
        true,
    );
    let rendered = reader::render(&source, 60);
    let visible = rendered
        .lines
        .iter()
        .map(ToString::to_string)
        .filter(|row| !row.is_empty())
        .collect::<Vec<_>>();
    assert_eq!(
        visible,
        [
            "• Inspect queues",
            "  Then check reservations.",
            "let capacity = 42;",
            "• Inspect usage",
            "Outside the list.",
        ]
    );
    let code = rendered
        .code
        .iter()
        .position(|code| *code)
        .unwrap_or(usize::MAX);
    assert_eq!(rendered.insets.get(code), Some(&2));
}

#[test]
fn items_containing_only_a_block_keep_the_marker_before_that_block() {
    for source in [
        "- ![Queue diagram](https://example.org/queue.png)",
        "- ```rust\n  let capacity = 42;\n  ```",
        "- | Name | Value |\n  | --- | --- |\n  | Queue | Ready |",
        "+ ---",
    ] {
        let rendered = reader::render(source, 40);
        let visible = rendered
            .lines
            .iter()
            .map(ToString::to_string)
            .filter(|row| !row.is_empty())
            .collect::<Vec<_>>();
        assert_eq!(visible.first().map(String::as_str), Some("•"), "{source}");
        assert_ne!(visible.last().map(String::as_str), Some("•"), "{source}");
    }
}
