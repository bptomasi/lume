//! Native reader operations use real parser ranges and application key paths.

mod common;

use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lume::{
    app::{Action, App, Focus, Mode},
    reader::index::DocumentIndex,
    source,
};

const BODY: &str = r#"# Overview

Alpha βeta alpha.

## Recovery

Alpha survives a restart.

```sql
SELECT id
FROM journal;
```

| Name | Value |
| --- | --- |
| epoch | 42 |
"#;

fn reading() -> Result<(tempfile::TempDir, App)> {
    reading_body(BODY)
}

fn reading_body(body: &str) -> Result<(tempfile::TempDir, App)> {
    let (directory, mut store, feed) = common::seeded_store()?;
    let mut parsed =
        source::parse(include_bytes!("fixtures/rss.xml"), &feed.url)?;
    parsed.articles[0].body = body.into();
    store.ingest(&feed, &parsed.title, &parsed.articles)?;
    let mut app = App::new(store, false)?;
    app.focus = Focus::Reader;
    app.resize_reader(70, 12);
    Ok((directory, app))
}

fn key(app: &mut App, code: KeyCode) -> Result<Action> {
    app.key(KeyEvent::new(code, KeyModifiers::NONE))
}

fn command(app: &mut App, text: &str) -> Result<Action> {
    key(app, KeyCode::Char(':'))?;
    app.paste(text)?;
    key(app, KeyCode::Enter)
}

#[test]
fn parser_indexes_heading_levels_exact_code_and_table_source() -> Result<()> {
    let index = DocumentIndex::parse(BODY);
    assert_eq!(
        index
            .headings
            .iter()
            .map(|heading| (heading.level, heading.title.as_str()))
            .collect::<Vec<_>>(),
        vec![(1, "Overview"), (2, "Recovery")]
    );
    assert_eq!(index.code[0].text, "SELECT id\nFROM journal;\n");
    assert_eq!(
        BODY.get(index.tables[0].clone()).context("Table range")?,
        "| Name | Value |\n| --- | --- |\n| epoch | 42 |\n"
    );
    Ok(())
}

#[test]
fn reader_search_does_not_filter_library_and_matches_survive_reflow()
-> Result<()> {
    let (_directory, mut app) = reading()?;
    let total = app.page_total;
    command(&mut app, "find Alpha")?;
    assert_eq!(app.reader_tools.hits.len(), 2);
    let first = app.cursor_offset();
    command(&mut app, "find-next")?;
    assert_ne!(app.cursor_offset(), first);
    app.resize_reader(15, 5);
    assert_eq!(app.reader_tools.hits.len(), 2);
    command(&mut app, "find-prev")?;
    assert_eq!(app.cursor_offset(), first);
    assert_eq!(app.page_total, total);
    assert!(app.search.is_empty());
    Ok(())
}

#[test]
fn outline_history_and_code_yank_retain_source_bytes() -> Result<()> {
    let (_directory, mut app) = reading()?;
    command(&mut app, "toc")?;
    let Mode::Picker(picker) = &app.mode else {
        anyhow::bail!("Outline did not open");
    };
    assert_eq!(picker.choices.len(), 2);
    key(&mut app, KeyCode::Down)?;
    key(&mut app, KeyCode::Enter)?;
    assert_eq!(app.reader_lines[app.reader_cursor], "Recovery");
    app.resize_reader(15, 5);
    assert_eq!(app.reader_lines[app.reader_cursor], "Recovery");
    command(&mut app, "jump-back")?;
    assert_eq!(app.reader_lines[app.reader_cursor], "Overview");
    command(&mut app, "jump-forward")?;
    assert_eq!(app.reader_lines[app.reader_cursor], "Recovery");
    assert_eq!(
        command(&mut app, "copy-code")?,
        Action::Copy("SELECT id\nFROM journal;\n".into())
    );
    Ok(())
}

#[test]
fn html_tables_keep_cells_links_and_a_safe_wide_view() -> Result<()> {
    let html = r#"<table><tr><th>Name</th><th>Value</th></tr><tr><td>a|b</td><td><a href="/reference">Reference</a></td></tr></table>"#;
    let markdown = lume::content::article_markdown(
        html,
        true,
        "https://journal.example/a",
    );
    let index = DocumentIndex::parse(&markdown);
    assert_eq!(index.tables.len(), 1);
    assert!(markdown.contains("a\\|b"));
    assert!(markdown.contains("https://journal.example/reference"));
    let view = lume::reader::table_view::render(&markdown);
    assert!(view.contains("a|b") && view.contains("Reference"));
    assert!(view.starts_with('╭'));
    assert!(view.lines().any(|line| line.starts_with('├')));
    assert!(view.trim_end().ends_with('╯'));
    Ok(())
}

#[test]
fn wide_table_view_scrolls_without_mutating_article_or_search() -> Result<()> {
    let (_directory, mut app) = reading()?;
    command(&mut app, "tables")?;
    key(&mut app, KeyCode::Right)?;
    key(&mut app, KeyCode::Down)?;
    let Mode::Document(document) = &app.mode else {
        anyhow::bail!("Table viewport did not open");
    };
    assert_eq!((document.column, document.row), (8, 1));
    assert!(document.lines.iter().any(|line| line.contains("epoch")));
    key(&mut app, KeyCode::Esc)?;
    assert_eq!(app.body, BODY);
    assert_eq!(
        command(&mut app, "copy-code")?,
        Action::Copy("SELECT id\nFROM journal;\n".into())
    );
    Ok(())
}

#[test]
fn inspector_keeps_tables_that_exceed_grid_limits_alongside_cached_grids()
-> Result<()> {
    let body =
        format!("{BODY}\n| Wide |\n| --- |\n| {} END |\n", "x".repeat(4100));
    let (_directory, mut app) = reading_body(&body)?;
    assert_eq!(app.table_rows.len(), 1);
    assert_eq!(app.reader_tools.index.tables.len(), 2);

    command(&mut app, "tables")?;

    let Mode::Document(document) = &app.mode else {
        anyhow::bail!("Table viewport did not open");
    };
    assert!(document.lines.iter().any(|line| line.contains("epoch")));
    assert!(document.lines.iter().any(|line| line.contains("END")));
    assert_eq!(app.body, body);
    Ok(())
}

#[test]
fn table_selection_yank_highlight_and_reflow_keep_the_exact_cell() -> Result<()>
{
    let (_directory, mut app) = reading()?;
    command(&mut app, "find epoch")?;
    let start = app.cursor_offset();
    key(&mut app, KeyCode::Char('v'))?;
    for _ in 0..4 {
        key(&mut app, KeyCode::Char('l'))?;
    }

    assert_eq!(
        key(&mut app, KeyCode::Char('y'))?,
        Action::Copy("epoch".into())
    );
    key(&mut app, KeyCode::Esc)?;
    command(&mut app, "find epoch")?;
    key(&mut app, KeyCode::Char('v'))?;
    for _ in 0..4 {
        key(&mut app, KeyCode::Char('l'))?;
    }
    key(&mut app, KeyCode::Char('s'))?;
    app.resize_reader(8, 10);

    assert_eq!(
        app.comments.last().context("Missing highlight")?.quote,
        "epoch"
    );
    assert_eq!(app.reader_tools.hits[0].start, start);
    assert!(!app.table_rows.is_empty());
    command(&mut app, "find epoch")?;
    key(&mut app, KeyCode::Char('$'))?;
    assert!(app.horizontal > 0, "Wide rows must pan with the cursor");
    assert_eq!(app.body, BODY);
    Ok(())
}

#[test]
fn personal_edits_and_refresh_keep_the_exact_cursor_in_unchanged_text()
-> Result<()> {
    let (_directory, mut app) = reading()?;
    command(&mut app, "find Alpha")?;
    command(&mut app, "find-next")?;
    for _ in 0..3 {
        key(&mut app, KeyCode::Char('l'))?;
    }
    let before = (app.cursor_offset(), app.scroll);

    command(&mut app, "tags recovery")?;
    assert_eq!((app.cursor_offset(), app.scroll), before);
    app.reload()?;
    assert_eq!((app.cursor_offset(), app.scroll), before);
    app.resize_reader(70, 12);
    assert_eq!((app.cursor_offset(), app.scroll), before);
    app.resize_reader(35, 6);
    assert_eq!(app.cursor_offset(), before.0);
    Ok(())
}

#[test]
fn wide_table_previews_enforce_the_byte_limit_in_aligned_and_raw_paths() {
    let header = "| Name |\n| --- |\n";
    let aligned = format!(
        "{header}{}",
        format!("| {} |\n", "界".repeat(2000)).repeat(200)
    );
    let raw = format!(
        "{header}{}",
        format!("| {} |\n", "界".repeat(200)).repeat(2001)
    );
    for source in [aligned, raw] {
        let view = lume::reader::table_view::render(&source);
        assert!(
            view.len() <= 1024 * 1024,
            "Preview exceeded its byte budget"
        );
        assert!(view.contains('界'), "The readable prefix was discarded");
        assert!(view.ends_with(
            "[Table preview limited to 1 MiB; the article is unchanged.]\n"
        ));
    }
}

#[test]
fn refresh_retains_article_and_note_identity_beyond_the_loaded_page()
-> Result<()> {
    let (directory, mut app) = reading()?;
    let original = app.current().context("Missing article")?.clone();
    let mut writer =
        lume::store::Store::open(&directory.path().join("library.sqlite3"))?;
    writer.save_comment(original.id, None, "First private note")?;
    writer.save_comment(original.id, None, "Selected private note")?;
    app.reload()?;
    app.comment_selected = 1;
    let selected_note = app.comments[1].id;
    app.page_size = 1;
    let feed = writer.feeds()?.remove(0);
    let mut incoming =
        source::parse(include_bytes!("fixtures/rss.xml"), &feed.url)?.articles;
    for index in 0..3 {
        incoming[0].external_id = format!("new-{index}");
        incoming[0].published = original.published + index + 1;
        writer.ingest(&feed, "Journal", &incoming)?;
    }

    app.reload()?;

    assert_eq!(app.current().map(|item| item.id), Some(original.id));
    assert_eq!((app.page_offset, app.library.articles.len()), (3, 1));
    assert_eq!(app.comments[app.comment_selected].id, selected_note);
    assert_eq!(app.body, BODY);
    Ok(())
}

#[test]
fn enter_follows_local_links_and_history_survives_reflow() -> Result<()> {
    let body = "[jump here](#recovery)\n\n# Overview\n\nA paragraph.\n\n\
        ## Recovery\n\nRecovered text.";
    let (_directory, mut app) = reading_body(body)?;
    let before = app.cursor_offset();

    assert_eq!(key(&mut app, KeyCode::Enter)?, Action::None);

    assert_eq!(app.reader_lines[app.reader_cursor], "Recovery");
    app.resize_reader(12, 5);
    app.key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL))?;
    assert_eq!(app.cursor_offset(), before);
    assert_eq!(app.cursor_link(), Some("#recovery"));
    app.key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::CONTROL))?;
    assert_eq!(app.reader_lines[app.reader_cursor], "Recovery");
    Ok(())
}

#[test]
fn enter_opens_the_exact_external_occurrence_in_a_wrapped_label() -> Result<()>
{
    let body = "Prefix [same long label](https://example.org/first) \
        and [same long label](https://example.org/second).";
    let (_directory, mut app) = reading_body(body)?;
    app.resize_reader(14, 5);
    command(&mut app, "find label")?;

    assert_eq!(key(&mut app, KeyCode::Enter)?, Action::None);
    assert!(app.web_active());
    key(&mut app, KeyCode::Esc)?;
    assert!(!app.web_active());
    assert_eq!(app.cursor_link(), Some("https://example.org/first"));

    command(&mut app, "find-next")?;
    assert_eq!(app.cursor_link(), Some("https://example.org/second"));
    assert_eq!(key(&mut app, KeyCode::Enter)?, Action::None);
    assert!(app.web_active());
    Ok(())
}

#[test]
fn same_page_urls_jump_and_missing_sections_do_not_open_an_unrelated_page()
-> Result<()> {
    let body = "[go](https://journal.example/cache#target)\n\n\
        ## Target\n\nTarget text.";
    let (_directory, mut app) = reading_body(body)?;

    assert_eq!(key(&mut app, KeyCode::Enter)?, Action::None);
    assert_eq!(app.reader_lines[app.reader_cursor], "Target");

    let (_directory, mut app) = reading_body("[go](#absent)\n\nText.")?;
    let before = app.cursor_offset();
    let error = key(&mut app, KeyCode::Enter)
        .err()
        .context("Missing section succeeded")?;
    assert!(error.to_string().contains("absent from the cached article"));
    assert_eq!(app.cursor_offset(), before);
    Ok(())
}

#[test]
fn html_anchor_links_jump_to_the_publishers_exact_id() -> Result<()> {
    let body = lume::content::article_markdown(
        "<p><a href='#custom-id'>go</a></p>\
        <h2 id='custom-id'>Different title</h2><p>Section text.</p>",
        true,
        "https://journal.example/cache",
    );
    let (_directory, mut app) = reading_body(&body)?;

    key(&mut app, KeyCode::Enter)?;

    assert_eq!(app.reader_lines[app.reader_cursor], "Different title");
    assert!(!app.reader_text.contains("<a"));
    Ok(())
}

#[test]
fn open_link_and_enter_share_the_readers_local_navigation() -> Result<()> {
    let (_directory, mut app) =
        reading_body("[Jump](#target)\n\n## Target\n\nText.")?;
    command(&mut app, "open-link")?;
    assert_eq!(app.reader_lines[app.reader_cursor], "Target");
    app.key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL))?;
    assert_eq!(app.cursor_offset(), 0);
    key(&mut app, KeyCode::Enter)?;
    assert_eq!(app.reader_lines[app.reader_cursor], "Target");
    Ok(())
}

#[test]
fn relative_markdown_destinations_are_resolved_against_the_opened_article()
-> Result<()> {
    for (target, expected) in [
        ("cache#target", "https://journal.example/cache#target"),
        (
            "../guide?lang=en#part",
            "https://journal.example/guide?lang=en#part",
        ),
        ("//docs.example/guide", "https://docs.example/guide"),
    ] {
        let body = format!("[Jump]({target})\n\n## Target\n\nText.");
        let (_directory, mut app) = reading_body(&body)?;
        assert_eq!(app.cursor_link(), Some(target));
        assert_eq!(
            source::resolve_link("https://journal.example/cache", target)?
                .as_str(),
            expected
        );
        command(&mut app, "open-link")?;
        if target == "cache#target" {
            assert_eq!(app.reader_lines[app.reader_cursor], "Target");
            assert!(!app.web_active());
        } else {
            assert!(app.web_active());
        }
    }
    for target in [
        "//user:password@example.org/a",
        "javascript:alert(1)",
        "file:///etc/passwd",
        "https://example.org/\n",
    ] {
        assert!(
            source::resolve_link("https://journal.example/cache", target)
                .is_err()
        );
    }
    Ok(())
}

#[test]
fn unsafe_links_and_plain_text_never_emit_browser_actions() -> Result<()> {
    for body in [
        "[blocked](javascript:alert)",
        "[blocked](file:///etc/passwd)",
        "Plain text.",
    ] {
        let (_directory, mut app) = reading_body(body)?;
        assert_eq!(app.cursor_link(), None);
        assert_eq!(key(&mut app, KeyCode::Enter)?, Action::None);
    }
    Ok(())
}
