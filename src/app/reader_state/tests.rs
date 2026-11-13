//! Reader invariants are exercised without an application or storage adapter.

use super::{ReaderState, RememberedProgress, RenderContext};
use crate::{
    app::reading_data::ReadingData,
    model::{
        Article, CachedDocument, Comment, ContentSource, FeedOrigin,
        FetchPolicy,
    },
};
use anyhow::{Context, Result};

fn article(id: i64) -> Article {
    Article {
        id,
        feed_id: 1,
        title: "Publisher article".into(),
        url: "https://example.org/article".into(),
        feed: "Publisher".into(),
        author: String::new(),
        published: 0,
        unread: true,
        starred: false,
        comments: 0,
        queued: false,
    }
}

fn data(body: &str) -> ReadingData {
    ReadingData {
        article: 1,
        body: body.into(),
        origin: FeedOrigin::Feed,
        document: None,
        policy: FetchPolicy::Missing,
        comments: Vec::new(),
        progress: 0,
    }
}

fn context() -> RenderContext {
    RenderContext {
        annotated: false,
        immediate: true,
        justify: false,
        options: crate::reader::RenderOptions::default(),
    }
}

fn open(data: &mut ReadingData) -> Result<ReaderState> {
    let mut reader = ReaderState::new()?;
    reader.open_document(
        article(1),
        data,
        ContentSource::Full,
        context(),
        RememberedProgress::default(),
    )?;
    Ok(reader)
}

fn cached(url: &str) -> CachedDocument {
    CachedDocument {
        url: url.into(),
        final_url: url.into(),
        body: "Independent full publisher text.".into(),
        revision: 3,
        fetched_at: 0,
        attempt_url: url.into(),
        attempted_at: 0,
        error: String::new(),
        progress: 8,
    }
}

#[test]
fn malformed_article_transfer_cannot_replace_a_valid_projection() -> Result<()>
{
    let mut reader = open(&mut data("Original publisher text."))?;
    let mut incoming = data("A different source.");
    incoming.article = 2;
    let identity = reader.reader_identity();
    let text = reader.projection.text.clone();

    let result = reader.open_document(
        article(1),
        &mut incoming,
        ContentSource::Feed,
        context(),
        RememberedProgress::default(),
    );

    assert!(result.is_err());
    assert_eq!(reader.reader_identity(), identity);
    assert_eq!(reader.projection.text, text);
    assert_eq!(incoming.body, "A different source.");
    Ok(())
}

#[test]
fn cursor_and_selection_inputs_are_clamped_to_grapheme_boundaries() -> Result<()>
{
    let mut reader = open(&mut data("café e\u{301} 界"))?;
    reader.point_cursor(usize::MAX, usize::MAX);
    assert!(reader.viewport.row < reader.projection.lines.len());
    assert!(reader.cursor_line().is_char_boundary(reader.viewport.byte));
    reader.select_from(4, false);
    assert_eq!(reader.viewport.selection, Some(3));
    reader.place_cursor(usize::MAX);
    let selected = reader.selected_range().context("Selection missing")?;
    assert!(reader.projection.text.get(selected).is_some());
    reader.clear_current(context());
    assert_eq!(reader.reader_identity(), None);
    assert_eq!(
        (
            reader.viewport.first,
            reader.viewport.row,
            reader.viewport.byte
        ),
        (0, 0, 0)
    );
    assert!(reader.selected_range().is_none());
    Ok(())
}

#[test]
fn private_comment_text_cannot_be_captured_as_publisher_source() -> Result<()> {
    let mut incoming = data("Publisher passage with café.");
    incoming.comments.push(Comment {
        id: 7,
        body: "Private interpretation".into(),
        created: 0,
        quote: String::new(),
        anchor: 0,
        source: ContentSource::Feed,
        source_revision: 0,
    });
    let reader = open(&mut incoming)?;
    let publisher = reader.capture_passage(0..9)?;
    assert_eq!(publisher.quote, "Publisher");
    let private = reader
        .projection
        .text
        .find("Private interpretation")
        .context("Private comment absent")?;
    assert!(reader.capture_passage(private..private + 7).is_err());
    Ok(())
}

#[test]
fn full_cache_identity_and_progress_require_the_same_publisher_url()
-> Result<()> {
    let mut incoming = data("Feed text.");
    incoming.document = Some(cached("https://example.org/obsolete"));
    let mut reader = open(&mut incoming)?;
    assert_eq!(reader.source(), ContentSource::Feed);
    assert_eq!(reader.revision(), 0);
    assert_eq!(reader.publisher.body, "Feed text.");
    let mut incoming = data("Feed text.");
    incoming.document = Some(cached("https://example.org/article"));
    reader.open_document(
        article(1),
        &mut incoming,
        ContentSource::Full,
        context(),
        RememberedProgress {
            feed: Some(2),
            full: Some(14),
        },
    )?;
    assert_eq!(reader.source(), ContentSource::Full);
    assert_eq!(reader.revision(), 3);
    assert_eq!(
        reader.reading_position().map(|(_, anchor)| anchor),
        Some(14)
    );
    Ok(())
}

#[test]
fn snapshot_selection_does_not_cross_a_full_cache_revision() -> Result<()> {
    let mut incoming = data("Feed text.");
    incoming.document = Some(cached("https://example.org/article"));
    let mut reader = open(&mut incoming)?;
    reader.select_from(2, false);
    reader.place_cursor(10);
    let snapshot = reader.capture_reader();
    let mut incoming = data("Feed text.");
    let mut document = cached("https://example.org/article");
    document.revision = 4;
    incoming.document = Some(document);
    reader.open_document(
        article(1),
        &mut incoming,
        ContentSource::Full,
        context(),
        RememberedProgress::default(),
    )?;
    reader.restore_reader(snapshot, Some(1));
    assert_eq!(reader.revision(), 4);
    assert!(reader.viewport.selection.is_none());
    Ok(())
}

#[test]
fn width_reflow_preserves_publisher_selection_with_wrapped_private_notes()
-> Result<()> {
    let mut incoming =
        data(&"Publisher words with readable context. ".repeat(8));
    incoming.comments.push(Comment {
        id: 1,
        body: "A private comment whose display wrapping changes.".repeat(4),
        created: 0,
        quote: String::new(),
        anchor: 0,
        source: ContentSource::Feed,
        source_revision: 0,
    });
    let mut reader = open(&mut incoming)?;
    reader.resize_reader(72, 20, context(), true);
    reader.select_from(3, false);
    reader.place_cursor(21);
    let selection = reader.selected_range();
    reader.resize_reader(24, 8, context(), true);
    assert_eq!(reader.selected_range(), selection);
    assert_eq!(reader.cursor_offset(), 21);
    assert!(
        reader.viewport.first
            <= reader.projection.lines.len().saturating_sub(8)
    );
    Ok(())
}

#[test]
fn motions_over_a_retained_source_do_not_record_navigation_history()
-> Result<()> {
    let mut reader = open(&mut data("First paragraph.\n\nSecond paragraph."))?;
    reader.resize_reader(72, 20, context(), true);
    reader.goto_reader_row(1, false);
    assert!(reader.history_target(false).is_none());
    reader.goto_reader_row(0, true);
    assert_eq!(
        reader.history_target(false).map(|target| target.article),
        Some(1)
    );
    Ok(())
}

#[test]
fn a_snapshot_cannot_reinstate_selection_after_navigation_moves() -> Result<()>
{
    let mut reader = open(&mut data("Retained publisher passage."))?;
    reader.select_from(2, false);
    reader.place_cursor(9);
    let snapshot = reader.capture_reader();
    reader.cancel_selection();
    reader.restore_reader(snapshot, Some(2));
    assert_eq!(reader.reading_article(), Some(1));
    assert!(reader.selected_range().is_none());
    Ok(())
}

#[test]
fn a_snapshot_checks_private_text_at_both_selection_endpoints() -> Result<()> {
    let mut incoming = data("Publisher passage.");
    incoming.comments.push(Comment {
        id: 1,
        body: "Private interpretation with café.".into(),
        created: 0,
        quote: String::new(),
        anchor: 0,
        source: ContentSource::Feed,
        source_revision: 0,
    });
    let mut reader = open(&mut incoming)?;
    let private = reader
        .projection
        .text
        .find("Private interpretation")
        .context("Private comment absent")?;
    reader.select_from(private, false);
    reader.place_cursor(2);
    let snapshot = reader.capture_reader();
    reader.replace_comments(Vec::new());
    reader.wrap_reader(context());

    reader.restore_reader(snapshot, Some(1));

    assert!(reader.selected_range().is_none());
    assert!(!reader.projection.text.contains("Private interpretation"));
    Ok(())
}

#[test]
fn horizontal_pointer_scroll_refreshes_coverage_of_an_inactive_block()
-> Result<()> {
    let mut reader =
        open(&mut data("Intro.\n\n```text\nabcdefghijklmnopqrst\n```"))?;
    reader.resize_reader(10, 20, context(), true);
    let row = reader
        .projection
        .code
        .iter()
        .position(|code| *code)
        .context("Code block absent")?;
    reader.point_cursor(0, 0);
    reader.ensure_cursor_visible();
    let policy = crate::model::AutoReadSettings {
        enabled: true,
        min_active_seconds: 0,
        min_coverage_percent: 100,
    };
    let now = std::time::Instant::now();
    assert!(reader.observe_reading(now, policy, true).is_none());
    reader.scroll_horizontal_row(row, false);
    reader.resize_reader(10, 20, context(), true);
    assert!(reader.observe_reading(now, policy, true).is_none());
    let mut complete = None;
    for _ in 0..10 {
        reader.scroll_horizontal_row(row, true);
        complete = reader.observe_reading(now, policy, true);
    }

    assert_eq!(reader.viewport.horizontal, 0);
    assert_eq!(reader.horizontal_for_row(row), 10);
    assert_eq!(complete, reader.reader_identity());
    Ok(())
}

#[test]
fn a_wider_viewport_counts_only_fully_visible_code_row_ends() -> Result<()> {
    let policy = crate::model::AutoReadSettings {
        enabled: true,
        min_active_seconds: 0,
        min_coverage_percent: 100,
    };
    let now = std::time::Instant::now();
    for (line, narrow, full) in [
        ("abcdefghijklmnopqrst", 10, 20),
        ("abcdefghij界", 11, 12),
        ("abcdefghije\u{301}", 10, 11),
        ("abcdefghij👩‍💻", 11, 12),
    ] {
        let mut reader = open(&mut data(&format!("```text\n{line}\n```")))?;
        reader.resize_reader(narrow, 20, context(), true);
        assert!(
            reader.observe_reading(now, policy, true).is_none(),
            "{line}"
        );

        reader.resize_reader(full, 20, context(), true);

        assert_eq!(
            reader.observe_reading(now, policy, true),
            reader.reader_identity(),
            "{line}"
        );
    }
    Ok(())
}

#[test]
fn clearing_the_reader_discards_all_publisher_metadata() -> Result<()> {
    let mut incoming = data("Feed text.");
    incoming.origin = FeedOrigin::Summary;
    incoming.policy = FetchPolicy::Always;
    incoming.document = Some(cached("https://example.org/article"));
    let mut reader = open(&mut incoming)?;

    reader.clear_current(context());

    let view = reader.view();
    assert!(view.body.is_empty());
    assert!(view.document.is_none());
    assert_eq!(view.feed_origin, FeedOrigin::Feed);
    assert_eq!(view.fetch_policy, FetchPolicy::Missing);
    assert!(!reader.feed_has_text());
    assert!(reader.publisher.feed_links.is_empty());
    assert_eq!(view.content_source, ContentSource::Feed);
    assert_eq!(view.content_revision, 0);
    assert!(reader.reading_position().is_none());
    Ok(())
}

#[test]
fn a_left_clipped_wide_grapheme_does_not_count_as_read() -> Result<()> {
    let mut reader = open(&mut data("```text\n界\n```"))?;
    reader.resize_reader(1, 20, context(), true);
    let policy = crate::model::AutoReadSettings {
        enabled: true,
        min_active_seconds: 0,
        min_coverage_percent: 100,
    };
    let now = std::time::Instant::now();
    assert!(reader.observe_reading(now, policy, true).is_none());

    reader.scroll_horizontal_row(0, true);

    assert_eq!(reader.horizontal_for_row(0), 1);
    assert!(reader.observe_reading(now, policy, true).is_none());
    reader.resize_reader(2, 20, context(), true);
    assert_eq!(
        reader.observe_reading(now, policy, true),
        reader.reader_identity()
    );
    Ok(())
}

#[test]
fn clearing_the_reader_discards_selection_and_display_caches() -> Result<()> {
    let mut incoming = data(
        "Publisher [link](https://example.org).\n\n![Diagram](/image.svg)",
    );
    incoming.comments.push(Comment {
        id: 1,
        body: "Private interpretation.".into(),
        created: 0,
        quote: String::new(),
        anchor: 0,
        source: ContentSource::Feed,
        source_revision: 0,
    });
    let mut reader = open(&mut incoming)?;
    assert!(!reader.view().comments.is_empty());
    assert!(!reader.view().reader_mappings.is_empty());
    assert!(!reader.view().image_slots.is_empty());
    assert!(!reader.view().links.is_empty());
    reader.toggle_selection(true);

    reader.clear_current(context());

    let view = reader.view();
    assert!(view.comments.is_empty());
    assert!(view.annotations.is_empty());
    assert!(view.reader_text.is_empty());
    assert!(view.reader_mappings.is_empty());
    assert!(view.image_slots.is_empty());
    assert!(view.links.is_empty());
    assert_eq!((view.selection, view.linewise), (None, false));
    Ok(())
}
