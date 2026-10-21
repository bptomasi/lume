//! Notebook workflows use durable note identities across every pane and worker.

mod common;

use anyhow::{Context, Result, ensure};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lume::{
    app::{Action, App, Focus, InputKind, Mode},
    model::NoteKind,
    store::Store,
};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};

fn key(app: &mut App, code: KeyCode) -> Result<Action> {
    app.key(KeyEvent::new(code, KeyModifiers::NONE))
}

fn command(app: &mut App, text: &str) -> Result<Action> {
    key(app, KeyCode::Char(':'))?;
    app.paste(text)?;
    key(app, KeyCode::Enter)
}

fn input(app: &App) -> Result<&lume::app::Input> {
    let Mode::Input(input) = &app.mode else {
        anyhow::bail!("Expected a captured editor");
    };
    Ok(input)
}

fn annotated() -> Result<(tempfile::TempDir, App, Store, i64, i64)> {
    let (directory, store, _) = common::seeded_store()?;
    let article = store.library()?.articles[0].id;
    let first = store.save_comment(article, None, "A consensus definition")?;
    let second = store.save_comment(
        article,
        None,
        &format!("A question about [[Note-{first}]]"),
    )?;
    store.set_note_metadata(
        article,
        second,
        &lume::model::NoteMetadata {
            kind: NoteKind::Question,
            ..Default::default()
        },
    )?;
    let reader = Store::open(&directory.path().join("library.sqlite3"))?;
    Ok((directory, App::new(store, false)?, reader, article, first))
}

#[test]
fn creation_requires_reader_even_when_a_source_article_is_selected()
-> Result<()> {
    let (_directory, mut app, store, article, _) = annotated()?;
    for focus in [Focus::Library, Focus::Articles, Focus::Annotations] {
        app.focus = focus;
        assert!(command(&mut app, "note").is_err());
        assert!(matches!(input(&app)?.kind, InputKind::Command));
        key(&mut app, KeyCode::Esc)?;
        assert!(key(&mut app, KeyCode::Char('n')).is_err());
        assert!(matches!(app.mode, Mode::Normal));
    }
    app.focus = Focus::Reader;
    command(&mut app, "note")?;
    assert!(matches!(
        input(&app)?.kind,
        InputKind::Comment { comment: None, .. }
    ));
    assert_eq!(store.comments(article)?.len(), 2);
    Ok(())
}

#[test]
fn global_search_targets_one_note_and_source_jump_keeps_its_identity()
-> Result<()> {
    let (_directory, mut app, store, article, _) = annotated()?;
    command(&mut app, "notes type:question body:question")?;
    common::settle_notes(&mut app)?;
    let notebook = app.notebook.as_ref().context("Notebook missing")?;
    assert_eq!(notebook.page.total, 1);
    let id = notebook.current().context("Note missing")?.id;
    assert!(
        notebook
            .current()
            .context("Note missing")?
            .preview
            .contains('\u{1e}')
    );
    key(&mut app, KeyCode::Enter)?;
    assert!(app.notebook.is_none());
    assert_eq!(app.focus, Focus::Reader);
    assert_eq!(app.current().context("Article missing")?.id, article);
    assert_eq!(app.comments[app.comment_selected].id, id);
    assert_eq!(store.comments(article)?.len(), 2);
    app.key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL))?;
    common::settle_notes(&mut app)?;
    assert_eq!(
        app.notebook
            .as_ref()
            .context("Notebook return missing")?
            .query,
        "type:question body:question"
    );
    Ok(())
}

#[test]
fn search_cancel_saved_collections_and_reference_history_preserve_the_query()
-> Result<()> {
    let (_directory, mut app, _store, _, first) = annotated()?;
    command(&mut app, "notes type:question")?;
    common::settle_notes(&mut app)?;
    command(&mut app, "note-collection Questions")?;
    command(&mut app, "note-backlinks")?;
    common::settle_notes(&mut app)?;
    key(&mut app, KeyCode::Enter)?;
    common::settle_notes(&mut app)?;
    assert_eq!(
        app.notebook.as_ref().context("Notebook missing")?.query,
        format!("id:{first}")
    );
    app.key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL))?;
    common::settle_notes(&mut app)?;
    key(&mut app, KeyCode::Char('/'))?;
    app.paste(" NOT tag:consensus")?;
    key(&mut app, KeyCode::Esc)?;
    common::settle_notes(&mut app)?;
    let notebook = app.notebook.as_ref().context("Notebook missing")?;
    assert_eq!(notebook.query, "type:question");
    assert_eq!(notebook.collections[0].name, "Questions");
    Ok(())
}

#[test]
fn query_cursor_motion_keeps_the_selected_note_and_ready_context() -> Result<()>
{
    let (_directory, mut app, _store, _, _) = annotated()?;
    command(&mut app, "notes body:A")?;
    common::settle_notes(&mut app)?;
    key(&mut app, KeyCode::Char('j'))?;
    common::settle_notes(&mut app)?;
    let selected = app
        .notebook
        .as_ref()
        .and_then(|book| book.current())
        .context("Second note missing")?
        .id;
    key(&mut app, KeyCode::Char('/'))?;

    for code in [KeyCode::Left, KeyCode::Right, KeyCode::Home, KeyCode::End] {
        key(&mut app, code)?;
        let book = app.notebook.as_ref().context("Notebook closed")?;
        assert_eq!(book.selected, 1);
        assert_eq!(book.current().context("Selection lost")?.id, selected);
        assert!(book.current_context().is_some());
        assert!(!app.notebook_pending());
    }
    Ok(())
}

#[test]
fn deleting_and_undoing_a_notebook_row_never_deletes_its_article() -> Result<()>
{
    let (_directory, mut app, store, article, first) = annotated()?;
    command(&mut app, &format!("notes id:{first}"))?;
    common::settle_notes(&mut app)?;
    key(&mut app, KeyCode::Char('d'))?;
    key(&mut app, KeyCode::Char('d'))?;
    common::settle_notes(&mut app)?;
    assert!(store.annotation(article, first).is_err());
    assert!(!store.is_deleted(article)?);
    key(&mut app, KeyCode::Char('u'))?;
    common::settle_notes(&mut app)?;
    assert_eq!(
        app.notebook
            .as_ref()
            .context("Notebook missing")?
            .page
            .total,
        1
    );
    assert!(store.annotation(article, first).is_ok());
    Ok(())
}

#[test]
fn note_templates_search_and_reference_completion_preserve_the_draft()
-> Result<()> {
    let (_directory, mut app, store, article, _) = annotated()?;
    app.focus = Focus::Reader;
    command(&mut app, "note")?;
    key(&mut app, KeyCode::Esc)?;
    command(&mut app, "template question")?;
    app.paste("What guarantees ordering? ")?;
    key(&mut app, KeyCode::Char('['))?;
    key(&mut app, KeyCode::Char('['))?;
    common::settle_notes(&mut app)?;
    key(&mut app, KeyCode::Esc)?;
    assert!(input(&app)?.text.ends_with("[["));
    key(&mut app, KeyCode::Backspace)?;
    key(&mut app, KeyCode::Backspace)?;
    key(&mut app, KeyCode::Esc)?;
    command(&mut app, "link consensus")?;
    common::settle_notes(&mut app)?;
    key(&mut app, KeyCode::Enter)?;
    key(&mut app, KeyCode::Esc)?;
    command(&mut app, "find ordering")?;
    assert!(input(&app)?.editor.search_pattern().is_some());
    command(&mut app, "wq")?;
    let saved = store.comments(article)?.pop().context("New note missing")?;
    let metadata = store.annotation(article, saved.id)?.metadata;
    assert_eq!(metadata.kind, NoteKind::Question);
    assert_eq!(metadata.references.len(), 1);
    Ok(())
}

#[test]
fn interrupted_drafts_are_recoverable_and_stale_writes_are_rejected()
-> Result<()> {
    let (directory, mut app, store, article, first) = annotated()?;
    app.focus = Focus::Reader;
    app.comment_selected = 0;
    command(&mut app, "edit-note")?;
    app.paste(" with local edits")?;
    app.checkpoint_draft()?;
    store.save_comment(article, Some(first), "An external revision")?;
    drop(app);
    let mut app = App::new(
        Store::open(&directory.path().join("library.sqlite3"))?,
        false,
    )?;
    assert!(app.status.contains("recoverable"));
    command(&mut app, "drafts")?;
    common::settle_notes(&mut app)?;
    key(&mut app, KeyCode::Enter)?;
    assert!(input(&app)?.text.contains("local edits"));
    key(&mut app, KeyCode::Esc)?;
    assert!(command(&mut app, "w").is_err());
    assert_eq!(store.comments(article)?[0].body, "An external revision");
    assert_eq!(store.note_draft_count()?, 1);
    key(&mut app, KeyCode::Esc)?;
    command(&mut app, "q!")?;
    assert_eq!(store.note_draft_count()?, 0);
    Ok(())
}

#[test]
fn writes_do_not_block_input_or_close_newer_text_while_sqlite_is_busy()
-> Result<()> {
    let (directory, store, _) = common::seeded_store()?;
    let path = directory.path().join("library.sqlite3");
    let mut app = App::new(store, false)?;
    app.enable_background_navigation(&path)?;
    app.focus = Focus::Reader;
    command(&mut app, "note")?;
    app.paste("First revision")?;
    key(&mut app, KeyCode::Esc)?;
    let lock = rusqlite::Connection::open(&path)?;
    lock.execute_batch("BEGIN IMMEDIATE")?;
    let start = std::time::Instant::now();
    command(&mut app, "wq")?;
    std::thread::sleep(std::time::Duration::from_millis(20));
    app.poll_note_tasks()?;
    key(&mut app, KeyCode::Char('A'))?;
    app.paste(" with newer text")?;
    ensure!(
        start.elapsed() < std::time::Duration::from_secs(1),
        "Input waited for the database lock"
    );
    lock.execute_batch("ROLLBACK")?;
    common::settle_notes(&mut app)?;
    assert!(input(&app)?.modified());
    let reader = Store::open(&path)?;
    let article = app.current().context("Article missing")?.id;
    assert_eq!(reader.comments(article)?[0].body, "First revision");
    key(&mut app, KeyCode::Esc)?;
    command(&mut app, "wq")?;
    common::settle_notes(&mut app)?;
    assert!(matches!(app.mode, Mode::Normal));
    let saved = reader.comments(article)?;
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].body, "First revision with newer text");
    Ok(())
}

#[test]
fn notebook_frames_fit_themes_and_sizes_and_keep_the_reading_layout()
-> Result<()> {
    let (_directory, mut app, store, _, _) = annotated()?;
    let reading = lume::model::PanelLayout::capture(&app.workspace);
    command(&mut app, "notebook")?;
    common::settle_notes(&mut app)?;
    for theme in ["lume", "nord", "gruvbox"] {
        app.workspace.theme = theme.into();
        for (width, height) in [(1, 1), (40, 18), (80, 28), (144, 44)] {
            app.resize_workspace(Rect::new(0, 0, width, height));
            common::settle_notes(&mut app)?;
            let mut terminal = Terminal::new(TestBackend::new(width, height))?;
            terminal.draw(|frame| lume::ui::draw(frame, &app))?;
            assert_eq!(terminal.backend().buffer().area.width, width);
        }
    }
    app.focus = Focus::Articles;
    command(&mut app, "panel-close")?;
    app.save_session()?;
    let saved = store.workspace()?;
    assert_eq!(lume::model::PanelLayout::capture(&saved), reading);
    assert!(
        !saved
            .notebook_layout
            .context("Notebook layout missing")?
            .visible[1]
    );
    key(&mut app, KeyCode::Esc)?;
    assert_eq!(lume::model::PanelLayout::capture(&app.workspace), reading);
    Ok(())
}

#[test]
fn batched_keyboard_and_mouse_motion_navigate_notes_without_moving_articles()
-> Result<()> {
    use crossterm::event::{Event, MouseEvent, MouseEventKind};
    let (_directory, mut app, _store, article, _) = annotated()?;
    command(&mut app, "notebook")?;
    common::settle_notes(&mut app)?;
    app.resize_workspace(Rect::new(0, 0, 144, 44));
    let events =
        vec![
            Event::Key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
            8
        ];
    let mut batch = lume::app::InputBatch::new(events);
    batch.dispatch_next(&mut app)?;
    common::settle_notes(&mut app)?;
    assert_eq!(
        app.notebook.as_ref().context("Notebook missing")?.selected,
        1
    );
    assert_eq!(app.current().context("Article missing")?.id, article);
    let area = lume::ui::workspace_areas(
        Rect::new(0, 0, 144, 44),
        app.focus,
        &app.workspace,
    )
    .articles
    .context("Notes pane missing")?;
    let event = MouseEvent {
        kind: MouseEventKind::ScrollUp,
        row: area.y + 2,
        column: area.x + 2,
        modifiers: KeyModifiers::NONE,
    };
    app.mouse(event)?;
    common::settle_notes(&mut app)?;
    assert_eq!(
        app.notebook.as_ref().context("Notebook missing")?.selected,
        0
    );
    assert_eq!(app.current().context("Article missing")?.id, article);
    Ok(())
}

#[test]
fn marked_notes_form_a_typed_synthesis_with_stable_evidence_links() -> Result<()>
{
    let (_directory, mut app, store, article, _) = annotated()?;
    command(&mut app, "notebook")?;
    common::settle_notes(&mut app)?;
    key(&mut app, KeyCode::Char('v'))?;
    key(&mut app, KeyCode::Char('j'))?;
    common::settle_notes(&mut app)?;
    key(&mut app, KeyCode::Char('v'))?;
    key(&mut app, KeyCode::Char('s'))?;
    common::settle_notes(&mut app)?;
    assert!(input(&app)?.text.contains("## Evidence"));
    key(&mut app, KeyCode::Esc)?;
    command(&mut app, "wq")?;
    let saved = store
        .comments(article)?
        .pop()
        .context("Synthesis missing")?;
    let metadata = store.annotation(article, saved.id)?.metadata;
    assert_eq!(metadata.kind, NoteKind::Synthesis);
    assert_eq!(metadata.references.len(), 2);
    Ok(())
}

#[test]
fn card_review_hides_the_answer_and_records_only_an_explicit_grade()
-> Result<()> {
    let (_directory, mut app, store, article, first) = annotated()?;
    store.create_study_card(
        article,
        first,
        "Who owns ordering?",
        "The transaction owner",
    )?;
    command(&mut app, "cards body:consensus")?;
    common::settle_notes(&mut app)?;
    let Mode::Picker(picker) = &app.mode else {
        anyhow::bail!("Expected due questions");
    };
    assert_eq!(picker.choices.len(), 1);
    assert!(!picker.preview.contains("The transaction owner"));
    key(&mut app, KeyCode::Enter)?;
    let Mode::Document(document) = &app.mode else {
        anyhow::bail!("Expected revealed answer");
    };
    assert!(
        document
            .lines
            .iter()
            .any(|line| line.contains("The transaction owner"))
    );
    assert_eq!(store.annotation(article, first)?.cards[0].reviews, 0);
    key(&mut app, KeyCode::Enter)?;
    for _ in 0..3 {
        key(&mut app, KeyCode::Down)?;
    }
    key(&mut app, KeyCode::Enter)?;
    let card = &store.annotation(article, first)?.cards[0];
    assert_eq!(card.reviews, 1);
    assert_eq!(card.interval_days, 14);
    Ok(())
}

#[test]
fn a_new_editor_cannot_replace_an_interrupted_buffer_for_the_same_note()
-> Result<()> {
    let (_directory, mut app, store, article, _) = annotated()?;
    let draft = lume::model::NoteDraft {
        target: lume::model::DraftTarget::Note {
            article,
            note: None,
        },
        body: "An interrupted observation".into(),
        baseline: String::new(),
        initial_kind: Some(NoteKind::Insight),
    };
    store.save_note_draft(&draft)?;
    app.focus = Focus::Reader;

    assert!(command(&mut app, "note").is_err());
    assert_eq!(store.note_drafts()?, vec![draft]);
    Ok(())
}

#[test]
fn version_restoration_previews_both_texts_and_is_reversible() -> Result<()> {
    let (_directory, mut app, store, article, first) = annotated()?;
    store.save_comment(
        article,
        Some(first),
        "A revised consensus definition",
    )?;
    command(&mut app, &format!("notes id:{first}"))?;
    common::settle_notes(&mut app)?;
    command(&mut app, "note-history")?;
    common::settle_notes(&mut app)?;
    key(&mut app, KeyCode::Down)?;
    key(&mut app, KeyCode::Enter)?;
    let Mode::Document(document) = &app.mode else {
        anyhow::bail!("Expected version confirmation");
    };
    assert!(
        document
            .lines
            .iter()
            .any(|line| line == "A revised consensus definition")
    );
    assert!(
        document
            .lines
            .iter()
            .any(|line| line == "A consensus definition")
    );
    assert_eq!(
        store.comments(article)?[0].body,
        "A revised consensus definition"
    );
    key(&mut app, KeyCode::Enter)?;
    assert_eq!(store.comments(article)?[0].body, "A consensus definition");
    store.undo()?;
    assert_eq!(
        store.comments(article)?[0].body,
        "A revised consensus definition"
    );
    Ok(())
}

#[test]
fn rapid_queued_property_changes_resolve_against_the_transactional_state()
-> Result<()> {
    let (directory, mut app, store, article, first) = annotated()?;
    app.enable_background_navigation(
        &directory.path().join("library.sqlite3"),
    )?;
    command(&mut app, &format!("notes id:{first}"))?;
    common::settle_notes(&mut app)?;
    key(&mut app, KeyCode::Char('p'))?;
    key(&mut app, KeyCode::Char('p'))?;
    key(&mut app, KeyCode::Char('r'))?;
    key(&mut app, KeyCode::Char('r'))?;
    common::settle_notes(&mut app)?;
    let metadata = store.annotation(article, first)?.metadata;
    assert!(!metadata.pinned);
    assert!(!metadata.resolved);
    Ok(())
}

#[test]
fn zen_reader_can_open_the_full_notebook_and_keep_shared_named_profiles()
-> Result<()> {
    let (_directory, mut app, store, _, _) = annotated()?;
    command(&mut app, "zen")?;
    let reading = lume::model::PanelLayout::capture(&app.workspace);
    command(&mut app, "notebook")?;
    common::settle_notes(&mut app)?;
    assert!(!app.workspace.zen);
    assert!(app.workspace.show_library && app.workspace.show_articles);
    command(&mut app, "workspace save My notebook")?;
    key(&mut app, KeyCode::Esc)?;
    assert_eq!(lume::model::PanelLayout::capture(&app.workspace), reading);
    app.save_session()?;
    assert!(
        store
            .layouts(&store.workspace()?)?
            .profiles
            .contains_key("My notebook")
    );
    Ok(())
}

#[test]
fn anchor_repair_shows_old_and_new_ranges_before_committing() -> Result<()> {
    let (_directory, mut app, store, article, first) = annotated()?;
    app.focus = Focus::Reader;
    app.comment_selected = 0;
    app.reader_cursor = 0;
    app.reader_column = 0;
    key(&mut app, KeyCode::Char('v'))?;
    key(&mut app, KeyCode::Char('$'))?;
    command(&mut app, "note-reanchor")?;
    assert!(matches!(app.mode, Mode::Document(_)));
    assert!(store.annotation(article, first)?.comment.quote.is_empty());
    key(&mut app, KeyCode::Enter)?;
    let quote = store.annotation(article, first)?.comment.quote;
    assert!(!quote.is_empty());
    assert!(store.body(article)?.contains(&quote));
    store.undo()?;
    assert!(store.annotation(article, first)?.comment.quote.is_empty());
    Ok(())
}

#[test]
fn notebook_commands_edit_the_visible_note_and_protect_hidden_article_selection()
-> Result<()> {
    let (_directory, mut app, mut store, original, _) = annotated()?;
    let feed = store.feeds()?.remove(0);
    store.ingest(
        &feed,
        &feed.title,
        &[lume::model::IncomingArticle {
            external_id: "other-source".into(),
            title: "Another source".into(),
            url: "https://journal.example/other".into(),
            author: "Ada".into(),
            published: 2_000_000_000,
            body: "Another source text".into(),
            body_origin: lume::model::FeedOrigin::Feed,
        }],
    )?;
    let other = store.library()?.articles[0].id;
    let note =
        store.save_comment(other, None, "Different source observation")?;
    command(&mut app, &format!("notes id:{note}"))?;
    common::settle_notes(&mut app)?;
    assert_eq!(
        app.current().context("Original selection missing")?.id,
        original
    );
    assert!(command(&mut app, "delete-article").is_err());
    key(&mut app, KeyCode::Esc)?;
    command(&mut app, "edit-note")?;
    assert!(matches!(input(&app)?.kind,
        InputKind::Comment { article, comment: Some(id) } if article == other && id == note));
    assert_eq!(input(&app)?.text, "Different source observation");
    assert!(!store.is_deleted(original)? && !store.is_deleted(other)?);
    Ok(())
}

#[test]
fn context_end_motion_keeps_a_full_final_page_and_clamps_further_scroll()
-> Result<()> {
    let (_directory, mut app, store, article, first) = annotated()?;
    store.save_comment(
        article,
        Some(first),
        &"Evidence deserves careful study.\n\n".repeat(60),
    )?;
    command(&mut app, &format!("notes id:{first}"))?;
    common::settle_notes(&mut app)?;
    app.focus = Focus::Reader;
    let screen = Rect::new(0, 0, 100, 24);
    app.resize_workspace(screen);
    common::settle_notes(&mut app)?;
    key(&mut app, KeyCode::Char('G'))?;
    let end = app.notebook.as_ref().context("Notebook missing")?.scroll;
    assert!(end > 0);
    key(&mut app, KeyCode::Char('j'))?;
    assert_eq!(
        app.notebook.as_ref().context("Notebook missing")?.scroll,
        end
    );
    let mut terminal = Terminal::new(TestBackend::new(100, 24))?;
    terminal.draw(|frame| lume::ui::draw(frame, &app))?;
    let area = lume::ui::workspace_areas(screen, app.focus, &app.workspace)
        .reader
        .context("Context missing")?;
    let mut rows = 0;
    for y in area.y + 1..area.bottom() - 1 {
        if (area.x + 1..area.right() - 1).any(|x| {
            terminal.backend().buffer()[(x, y)]
                .symbol()
                .chars()
                .any(char::is_alphanumeric)
        }) {
            rows += 1;
        }
    }
    assert!(rows >= 5, "End motion left a mostly blank context page");
    Ok(())
}

#[test]
fn backlinks_scroll_the_selected_reference_into_view() -> Result<()> {
    let (_directory, mut app, store, article, first) = annotated()?;
    let mut last = 0;
    for index in 0..40 {
        last = store.save_comment(
            article,
            None,
            &format!("Backlink {index} [[Note-{first}]]"),
        )?;
    }
    command(&mut app, &format!("notes id:{first}"))?;
    common::settle_notes(&mut app)?;
    command(&mut app, "panel-focus annotations")?;
    app.resize_workspace(Rect::new(0, 0, 120, 18));
    key(&mut app, KeyCode::Char('G'))?;
    let mut terminal = Terminal::new(TestBackend::new(120, 18))?;
    terminal.draw(|frame| lume::ui::draw(frame, &app))?;
    let text = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(
        text.contains("Backlink 39"),
        "Selected backlink was outside its viewport"
    );
    store.delete_comment(article, last)?;
    command(&mut app, &format!("notes id:{first}"))?;
    common::settle_notes(&mut app)?;
    let notebook = app.notebook.as_ref().context("Notebook missing")?;
    let context = notebook.current_context().context("Context missing")?;
    assert!(
        context.backlinks[notebook.link_selected]
            .body
            .contains("Backlink 38")
    );
    Ok(())
}

#[test]
fn widening_a_scrolled_context_reflows_and_clamps_without_losing_the_last_page()
-> Result<()> {
    let (_directory, mut app, store, article, first) = annotated()?;
    store.save_comment(
        article,
        Some(first),
        &"Evidence needs careful study of its context and source. ".repeat(180),
    )?;
    command(&mut app, &format!("notes id:{first}"))?;
    common::settle_notes(&mut app)?;
    app.focus = Focus::Reader;
    app.resize_workspace(Rect::new(0, 0, 80, 18));
    common::settle_notes(&mut app)?;
    key(&mut app, KeyCode::Char('G'))?;
    let old = app.notebook.as_ref().context("Notebook missing")?.scroll;
    app.resize_workspace(Rect::new(0, 0, 220, 38));
    common::settle_notes(&mut app)?;
    let notebook = app.notebook.as_ref().context("Notebook missing")?;
    assert!(
        notebook.scroll < old,
        "Reflow retained an obsolete row offset"
    );
    let mut terminal = Terminal::new(TestBackend::new(220, 38))?;
    terminal.draw(|frame| lume::ui::draw(frame, &app))?;
    let text = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(
        text.contains("SOURCE CONTEXT"),
        "The final context page became blank"
    );
    Ok(())
}

#[test]
fn ex_searches_enter_recent_history_and_custom_normal_bindings_work_in_notebook()
-> Result<()> {
    let (_directory, mut app, _store, _, _) = annotated()?;
    command(&mut app, "notes body:consensus")?;
    common::settle_notes(&mut app)?;
    command(&mut app, "bind ctrl-g note-search")?;
    app.key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL))?;
    assert!(matches!(input(&app)?.kind, InputKind::NoteSearch));
    assert_eq!(input(&app)?.text, "body:consensus");
    key(&mut app, KeyCode::Esc)?;
    common::settle_notes(&mut app)?;
    command(&mut app, "note-recent")?;
    let Mode::Picker(picker) = &app.mode else {
        anyhow::bail!("Expected recent search choices");
    };
    assert_eq!(picker.choices[0].title, "body:consensus");
    Ok(())
}

#[test]
fn pasted_code_keeps_tabs_and_trailing_newlines_through_save_and_yank()
-> Result<()> {
    let (_directory, mut app, store, article, _) = annotated()?;
    app.focus = Focus::Reader;
    command(&mut app, "note")?;
    let body = "\t[[Note-999999]]\n\tfn main() {}\n";
    app.paste(body)?;
    assert_eq!(input(&app)?.text, body);
    key(&mut app, KeyCode::Esc)?;
    command(&mut app, "wq")?;
    let note = store
        .comments(article)?
        .pop()
        .context("Saved note missing")?;
    assert_eq!(note.body, body);
    command(&mut app, &format!("notes id:{}", note.id))?;
    common::settle_notes(&mut app)?;
    assert_eq!(
        key(&mut app, KeyCode::Char('y'))?,
        Action::Copy(body.into())
    );
    Ok(())
}

#[test]
fn compatibility_note_commands_share_the_interactive_writer_and_export_owner()
-> Result<()> {
    let (directory, mut app, store, article, first) = annotated()?;
    app.enable_background_navigation(
        &directory.path().join("library.sqlite3"),
    )?;
    command(&mut app, &format!("notes id:{first}"))?;
    common::settle_notes(&mut app)?;
    command(&mut app, "highlight-tags consensus")?;
    common::settle_notes(&mut app)?;
    command(&mut app, "card Who owns ordering? | The transaction owner.")?;
    common::settle_notes(&mut app)?;
    command(&mut app, &format!("link-note {article}"))?;
    common::settle_notes(&mut app)?;
    let saved = store.annotation(article, first)?;
    assert_eq!(saved.tags, vec!["consensus"]);
    assert_eq!(saved.cards.len(), 1);
    assert_eq!(saved.cards[0].question, "Who owns ordering?");
    assert_eq!(saved.links, vec![article]);
    let output = directory.path().join("legacy-notes");
    command(&mut app, &format!("export-notes {}", output.display()))?;
    common::settle_notes(&mut app)?;
    assert!(output.join(format!("Lume-{article}.md")).exists());
    Ok(())
}
