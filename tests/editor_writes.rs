//! Vim-style writes preserve captured identities, drafts and literal paste.

mod common;

use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lume::{
    app::{Action, App, EditMode, Focus, Input, InputKind, Mode},
    store::Store,
    ui,
};
use ratatui::{Terminal, backend::TestBackend};

fn press(app: &mut App, code: KeyCode) -> Result<Action> {
    app.key(KeyEvent::new(code, KeyModifiers::NONE))
}

fn type_keys(app: &mut App, text: &str) -> Result<()> {
    for character in text.chars() {
        press(app, KeyCode::Char(character))?;
    }
    Ok(())
}

fn command(app: &mut App, text: &str) -> Result<Action> {
    press(app, KeyCode::Char(':'))?;
    app.paste(text)?;
    press(app, KeyCode::Enter)
}

fn input(app: &App) -> Result<&Input> {
    let Mode::Input(input) = &app.mode else {
        anyhow::bail!("The draft was closed");
    };
    Ok(input)
}

fn note_editor() -> Result<(tempfile::TempDir, App, Store, i64)> {
    let (directory, store, _feed) = common::seeded_store()?;
    let article = store.library()?.articles[0].id;
    let writer = Store::open(&directory.path().join("library.sqlite3"))?;
    let mut app = App::new(store, false)?;
    app.focus = lume::app::Focus::Reader;
    command(&mut app, "note")?;
    Ok((directory, app, writer, article))
}

#[test]
fn repeated_writes_update_one_captured_note_and_unchanged_writes_are_noops()
-> Result<()> {
    let (_directory, mut app, store, article) = note_editor()?;
    app.paste("First explanation")?;
    press(&mut app, KeyCode::Esc)?;
    assert!(input(&app)?.modified());

    assert_eq!(command(&mut app, "w")?, Action::None);
    let id = store.comments(article)?[0].id;
    assert_eq!(input(&app)?.edit_mode, EditMode::Normal);
    assert!(!input(&app)?.modified());
    store.clear_history()?;
    type_keys(&mut app, "A revised")?;
    press(&mut app, KeyCode::Esc)?;
    command(&mut app, "w")?;
    command(&mut app, "w!")?;

    let notes = store.comments(article)?;
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].id, id);
    assert_eq!(notes[0].body, "First explanation revised");
    assert!(store.undo()?);
    assert_eq!(store.comments(article)?[0].body, "First explanation");
    assert!(!store.undo()?);
    Ok(())
}

#[test]
fn write_and_close_aliases_save_without_quitting_the_application() -> Result<()>
{
    for alias in ["wq", "wq!", "x", "x!", "xit", "xit!", "exit", "exit!"] {
        let (_directory, mut app, store, article) = note_editor()?;
        app.paste("A private explanation")?;
        press(&mut app, KeyCode::Esc)?;

        let action = command(&mut app, alias)?;

        assert_eq!(action, Action::None, "{alias}");
        assert!(matches!(app.mode, Mode::Normal), "{alias}");
        assert_eq!(store.comments(article)?[0].body, "A private explanation");
    }
    Ok(())
}

#[test]
fn write_all_aliases_keep_the_single_editor_open() -> Result<()> {
    for alias in ["w", "w!", "write", "write!", "wa", "wa!", "wall", "wall!"] {
        let (_directory, mut app, store, article) = note_editor()?;
        app.paste("No duplicate buffers")?;
        press(&mut app, KeyCode::Esc)?;

        assert_eq!(command(&mut app, alias)?, Action::None, "{alias}");

        assert_eq!(input(&app)?.edit_mode, EditMode::Normal);
        assert_eq!(store.comments(article)?.len(), 1);
    }
    Ok(())
}

#[test]
fn write_all_quit_aliases_flush_note_and_session_before_returning_quit()
-> Result<()> {
    for alias in [
        "wqa", "wqa!", "wqall", "wqall!", "xa", "xa!", "xall", "xall!",
    ] {
        let (_directory, mut app, store, article) = note_editor()?;
        app.workspace.fullscreen_width = 94;
        app.paste("Saved before quitting")?;
        press(&mut app, KeyCode::Esc)?;

        assert_eq!(command(&mut app, alias)?, Action::Quit, "{alias}");

        assert!(matches!(app.mode, Mode::Normal));
        assert_eq!(store.comments(article)?[0].body, "Saved before quitting");
        assert_eq!(store.workspace()?.fullscreen_width, 94);
    }
    Ok(())
}

#[test]
fn quit_requires_a_clean_buffer_and_force_discards_only_the_draft() -> Result<()>
{
    for (quit, forced, expected) in [
        ("q", "q!", Action::None),
        ("quit", "quit!", Action::None),
        ("qa", "qa!", Action::Quit),
        ("qall", "qall!", Action::Quit),
    ] {
        let (_directory, mut app, store, article) = note_editor()?;
        app.paste("Do not lose this draft")?;
        press(&mut app, KeyCode::Esc)?;

        let error = command(&mut app, quit)
            .err()
            .context("Dirty quit succeeded")?;

        assert!(error.to_string().contains("Unsaved changes"));
        assert_eq!(input(&app)?.text, "Do not lose this draft");
        press(&mut app, KeyCode::Esc)?;
        assert_eq!(command(&mut app, forced)?, expected);
        assert!(store.comments(article)?.is_empty());
    }
    Ok(())
}

#[test]
fn quitting_a_clean_editor_does_not_create_an_empty_note() -> Result<()> {
    let (_directory, mut app, store, article) = note_editor()?;
    press(&mut app, KeyCode::Esc)?;

    assert_eq!(command(&mut app, "q")?, Action::None);

    assert!(matches!(app.mode, Mode::Normal));
    assert!(store.comments(article)?.is_empty());
    Ok(())
}

#[test]
fn failed_write_quit_keeps_the_draft_and_ex_command_for_correction()
-> Result<()> {
    let (_directory, mut app, store, article) = note_editor()?;
    press(&mut app, KeyCode::Esc)?;

    assert!(command(&mut app, "wqa!").is_err());

    assert_eq!(input(&app)?.edit_mode, EditMode::Command("wqa!".into()));
    assert!(store.comments(article)?.is_empty());
    press(&mut app, KeyCode::Esc)?;
    type_keys(&mut app, "iCorrected explanation")?;
    press(&mut app, KeyCode::Esc)?;
    assert_eq!(command(&mut app, "wqa")?, Action::Quit);
    assert_eq!(store.comments(article)?[0].body, "Corrected explanation");
    Ok(())
}

#[test]
fn deleted_note_identity_cannot_be_resurrected_by_a_later_write() -> Result<()>
{
    let (_directory, mut app, store, article) = note_editor()?;
    app.paste("Captured note")?;
    press(&mut app, KeyCode::Esc)?;
    command(&mut app, "w")?;
    let id = store.comments(article)?[0].id;
    store.delete_comment(article, id)?;
    type_keys(&mut app, "A with a pending edit")?;
    press(&mut app, KeyCode::Esc)?;

    assert!(command(&mut app, "wq!").is_err());

    assert_eq!(input(&app)?.text, "Captured note with a pending edit");
    assert!(matches!(input(&app)?.kind,
        InputKind::Comment { comment: Some(saved), .. } if saved == id));
    assert!(store.comments(article)?.is_empty());
    Ok(())
}

#[test]
fn inserted_and_pasted_ex_text_remains_literal_until_normal_mode() -> Result<()>
{
    let (_directory, mut app, store, article) = note_editor()?;
    type_keys(&mut app, ":wq")?;
    app.paste("\n:wqa\n:q!\u{1b}\u{7}")?;
    assert_eq!(input(&app)?.text, ":wq\n:wqa\n:q!");
    press(&mut app, KeyCode::Esc)?;
    press(&mut app, KeyCode::Char(':'))?;

    app.paste("wq\n")?;

    assert_eq!(input(&app)?.edit_mode, EditMode::Command("wq".into()));
    assert!(store.comments(article)?.is_empty());
    assert_eq!(press(&mut app, KeyCode::Enter)?, Action::None);
    assert_eq!(store.comments(article)?[0].body, ":wq\n:wqa\n:q!");
    Ok(())
}

#[test]
fn unknown_commands_and_unsupported_arguments_keep_both_buffers() -> Result<()>
{
    let (_directory, mut app, store, article) = note_editor()?;
    app.paste("Keep this explanation")?;
    press(&mut app, KeyCode::Esc)?;
    for command_text in ["not-a-command", "w notes.md", "wqa other", "q now"] {
        assert!(command(&mut app, command_text).is_err(), "{command_text}");
        assert_eq!(input(&app)?.text, "Keep this explanation");
        assert_eq!(
            input(&app)?.edit_mode,
            EditMode::Command(command_text.into())
        );
        press(&mut app, KeyCode::Esc)?;
    }
    assert!(store.comments(article)?.is_empty());
    Ok(())
}

#[test]
fn normal_motions_insert_undo_and_zz_keep_the_text_editor_responsible()
-> Result<()> {
    let (_directory, mut app, store, article) = note_editor()?;
    app.paste("café consensus\nsecond line")?;
    press(&mut app, KeyCode::Esc)?;
    type_keys(&mut app, "k0wiX")?;
    press(&mut app, KeyCode::Esc)?;
    assert_eq!(input(&app)?.text, "café Xconsensus\nsecond line");
    type_keys(&mut app, "u")?;
    assert_eq!(input(&app)?.text, "café consensus\nsecond line");

    type_keys(&mut app, "ZZ")?;

    assert!(matches!(app.mode, Mode::Normal));
    assert_eq!(
        store.comments(article)?[0].body,
        "café consensus\nsecond line"
    );
    Ok(())
}

#[test]
fn zq_discards_and_control_s_saves_in_each_editor_layer() -> Result<()> {
    let (_directory, mut app, store, article) = note_editor()?;
    app.paste("Discard me")?;
    press(&mut app, KeyCode::Esc)?;
    type_keys(&mut app, "ZQ")?;
    assert!(matches!(app.mode, Mode::Normal));
    assert!(store.comments(article)?.is_empty());
    for mode in [
        EditMode::Insert,
        EditMode::Normal,
        EditMode::Command("w".into()),
    ] {
        command(&mut app, "note")?;
        app.paste("Saved through Ctrl-s")?;
        if let Mode::Input(input) = &mut app.mode {
            input.edit_mode = mode;
        }
        let action =
            app.key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL))?;
        assert_eq!(action, Action::None);
        assert!(matches!(app.mode, Mode::Normal));
    }
    assert_eq!(store.comments(article)?.len(), 3);
    Ok(())
}

#[test]
fn a_passage_without_note_text_is_dirty_and_writes_keep_its_quote() -> Result<()>
{
    let (_directory, mut app, store, article) = note_editor()?;
    press(&mut app, KeyCode::Esc)?;
    command(&mut app, "q")?;
    app.focus = Focus::Reader;
    command(&mut app, "annotate")?;
    press(&mut app, KeyCode::Esc)?;
    assert!(input(&app)?.modified());
    assert!(command(&mut app, "q").is_err());
    press(&mut app, KeyCode::Esc)?;
    command(&mut app, "w")?;
    let original = store.comments(article)?[0].clone();
    assert!(!original.quote.is_empty());
    type_keys(&mut app, "iExplain the passage")?;
    press(&mut app, KeyCode::Esc)?;

    command(&mut app, "wq")?;

    let notes = store.comments(article)?;
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].id, original.id);
    assert_eq!(notes[0].quote, original.quote);
    assert_eq!(notes[0].anchor, original.anchor);
    assert_eq!(notes[0].source, original.source);
    assert_eq!(notes[0].body, "Explain the passage");
    Ok(())
}

#[test]
fn study_card_writes_update_one_card_and_preserve_its_review_schedule()
-> Result<()> {
    let (_directory, mut app, store, article) = note_editor()?;
    app.paste("A note for study")?;
    press(&mut app, KeyCode::Esc)?;
    command(&mut app, "wq")?;
    let comment = store.comments(article)?[0].id;
    command(&mut app, "card")?;
    app.paste("Who writes? | The owner")?;
    press(&mut app, KeyCode::Esc)?;
    command(&mut app, "w")?;
    let before = store.annotation(article, comment)?.cards[0].clone();
    store.review_study_card(&before, 7, 100)?;
    store.clear_history()?;
    type_keys(&mut app, "A of the transaction")?;
    press(&mut app, KeyCode::Esc)?;

    command(&mut app, "wq")?;

    let cards = store.annotation(article, comment)?.cards;
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].id, before.id);
    assert_eq!(cards[0].answer, "The owner of the transaction");
    assert_eq!((cards[0].due_at, cards[0].reviews), (604_900, 1));
    assert!(store.undo()?);
    assert_eq!(
        store.annotation(article, comment)?.cards[0].answer,
        "The owner"
    );
    Ok(())
}

#[test]
fn global_write_saves_session_and_write_quit_variants_request_exit()
-> Result<()> {
    let (_directory, mut app, store, _article) = note_editor()?;
    press(&mut app, KeyCode::Esc)?;
    command(&mut app, "q")?;
    app.workspace.fullscreen_width = 98;

    assert_eq!(command(&mut app, "w")?, Action::None);
    assert_eq!(store.workspace()?.fullscreen_width, 98);
    for alias in ["write!", "wa", "wall"] {
        assert_eq!(command(&mut app, alias)?, Action::None);
    }
    for alias in ["wq", "x", "exit", "wqa", "wqall", "xa", "xall", "qa!"] {
        assert_eq!(command(&mut app, alias)?, Action::Quit, "{alias}");
    }
    Ok(())
}

#[test]
fn command_line_is_visible_separately_and_never_overflows_tiny_viewports()
-> Result<()> {
    let (_directory, mut app, _store, _article) = note_editor()?;
    app.paste("Private draft text")?;
    press(&mut app, KeyCode::Esc)?;
    press(&mut app, KeyCode::Char(':'))?;
    app.paste("wqa")?;
    let mut terminal = Terminal::new(TestBackend::new(100, 30))?;

    terminal.draw(|frame| ui::draw(frame, &app))?;

    let text = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(text.contains("Private draft text"));
    assert!(text.contains("Private comment [+]"));
    assert!(text.contains(":wqa█"));
    assert!(text.contains("COMMAND"));
    assert_eq!(terminal.backend().buffer()[(1, 28)].symbol(), "C");
    assert_eq!(input(&app)?.text, "Private draft text");
    for (width, height) in [(1, 1), (24, 10), (40, 12)] {
        let mut terminal = Terminal::new(TestBackend::new(width, height))?;
        terminal.draw(|frame| ui::draw(frame, &app))?;
    }
    Ok(())
}

#[test]
fn ex_line_limit_and_backspace_preserve_unicode_and_the_note_buffer()
-> Result<()> {
    let (_directory, mut app, _store, _article) = note_editor()?;
    app.paste("Body")?;
    press(&mut app, KeyCode::Esc)?;
    press(&mut app, KeyCode::Char(':'))?;
    app.paste(&"é".repeat(200))?;
    assert_eq!(input(&app)?.edit_mode, EditMode::Command("é".repeat(128)));
    press(&mut app, KeyCode::Backspace)?;
    assert_eq!(input(&app)?.edit_mode, EditMode::Command("é".repeat(127)));
    app.key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL))?;
    app.paste("w")?;

    press(&mut app, KeyCode::Enter)?;

    assert_eq!(input(&app)?.text, "Body");
    assert_eq!(input(&app)?.edit_mode, EditMode::Normal);
    Ok(())
}

#[test]
fn session_failure_after_note_commit_keeps_the_saved_identity_for_retry()
-> Result<()> {
    let (directory, mut app, store, article) = note_editor()?;
    let injector =
        rusqlite::Connection::open(directory.path().join("library.sqlite3"))?;
    injector.execute_batch(include_str!(
        "../src/store/sql/test_create_reject_layout_history.sql"
    ))?;
    app.paste("Durable even if the session write fails")?;
    press(&mut app, KeyCode::Esc)?;

    assert!(command(&mut app, "wqa").is_err());

    let notes = store.comments(article)?;
    assert_eq!(notes.len(), 1);
    assert_eq!(input(&app)?.text, notes[0].body);
    assert!(!input(&app)?.modified());
    assert!(matches!(input(&app)?.kind,
        InputKind::Comment { comment: Some(id), .. } if id == notes[0].id));
    press(&mut app, KeyCode::Esc)?;
    command(&mut app, "w")?;
    assert_eq!(store.comments(article)?.len(), 1);
    Ok(())
}
