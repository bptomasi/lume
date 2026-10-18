//! Fullscreen shares reflow geometry and retains the underlying workspace.

mod common;

use anyhow::{Result, anyhow};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lume::{
    app::{App, Focus, Mode},
    model::Workspace,
    store::Store,
    ui,
};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};

fn press(app: &mut App, code: KeyCode) -> Result<()> {
    app.key(KeyEvent::new(code, KeyModifiers::NONE))?;
    Ok(())
}

fn text(app: &mut App, text: &str) -> Result<()> {
    for character in text.chars() {
        press(app, KeyCode::Char(character))?;
    }
    Ok(())
}

fn command(app: &mut App, value: &str) -> Result<()> {
    text(app, &format!(":{value}"))?;
    press(app, KeyCode::Enter)
}

#[test]
fn fullscreen_width_reflows_immediately_and_survives_restart_and_profiles()
-> Result<()> {
    let (directory, store, _) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    app.resize_workspace(Rect::new(0, 0, 200, 44));
    command(&mut app, "fullscreen")?;
    assert_eq!(app.reader_width, 80);

    command(&mut app, "fullscreen-width 100")?;
    assert_eq!(app.reader_width, 100);
    assert_eq!(app.workspace.reading_width, 82);
    command(&mut app, "workspace save Wide")?;
    command(&mut app, "fullscreen-width 60")?;
    assert_eq!(app.reader_width, 60);
    command(&mut app, "workspace load Wide")?;
    assert_eq!(app.reader_width, 100);
    app.end_session()?;
    drop(app);

    let restored = Store::open(&directory.path().join("library.sqlite3"))?;
    let mut app = App::new(restored, false)?;
    app.resize_workspace(Rect::new(0, 0, 200, 44));
    assert!(app.workspace.fullscreen);
    assert_eq!(app.reader_width, 100);
    Ok(())
}

#[test]
fn invalid_fullscreen_width_leaves_current_and_saved_preferences_intact()
-> Result<()> {
    let (directory, store, _) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    command(&mut app, "fullscreen-width 100")?;
    for invalid in ["0", "39", "141", "65536", "wide"] {
        assert!(
            command(&mut app, &format!("fullscreen-width {invalid}")).is_err()
        );
        assert_eq!(app.workspace.fullscreen_width, 100);
        press(&mut app, KeyCode::Esc)?;
    }
    let saved = Store::open(&directory.path().join("library.sqlite3"))?;
    assert_eq!(saved.workspace()?.fullscreen_width, 100);
    Ok(())
}

#[test]
fn older_workspace_and_profile_json_default_to_eighty_columns() -> Result<()> {
    let workspace: Workspace = serde_json::from_str("{}")?;
    assert_eq!(workspace.fullscreen_width, 80);
    let profile = lume::model::PanelLayout::capture(&workspace);
    let mut legacy = serde_json::to_value(profile)?;
    legacy
        .as_object_mut()
        .ok_or_else(|| anyhow!("Profile object"))?
        .remove("fullscreen_width");

    let restored: lume::model::PanelLayout = serde_json::from_value(legacy)?;
    assert_eq!(restored.fullscreen_width, 80);
    Ok(())
}

#[test]
fn narrow_settings_keep_the_selected_fullscreen_width_visible() -> Result<()> {
    let (_directory, store, _) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    app.resize_workspace(Rect::new(0, 0, 24, 10));
    command(&mut app, "settings")?;
    press(&mut app, KeyCode::Up)?;
    let mut terminal = Terminal::new(TestBackend::new(24, 10))?;
    terminal.draw(|frame| ui::draw(frame, &app))?;
    let rendered = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(rendered.contains("Fullscreen: 80"));
    Ok(())
}

#[test]
fn fullscreen_restores_focus_panel_preferences_and_selected_article()
-> Result<()> {
    let (_directory, store, _feed) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    app.focus = Focus::Library;
    app.workspace.article_width = 42;
    app.workspace.library_width = 28;
    app.workspace.reading_width = 65;
    app.workspace.show_articles = false;
    let article = app.current().map(|article| article.id);

    press(&mut app, KeyCode::F(11))?;
    assert!(app.workspace.fullscreen);
    assert_eq!(app.focus, Focus::Reader);
    press(&mut app, KeyCode::Esc)?;

    assert!(!app.workspace.fullscreen);
    assert_eq!(app.focus, Focus::Library);
    assert_eq!(app.workspace.article_width, 42);
    assert_eq!(app.workspace.library_width, 28);
    assert_eq!(app.workspace.reading_width, 65);
    assert!(!app.workspace.show_articles);
    assert_eq!(app.current().map(|article| article.id), article);
    Ok(())
}

#[test]
fn fullscreen_geometry_centers_a_bounded_column_without_changing_panel_width() {
    let workspace = Workspace {
        fullscreen: true,
        reading_width: 72,
        ..Workspace::default()
    };
    for (width, height) in [(24, 10), (80, 24), (144, 44), (240, 70)] {
        let screen = Rect::new(4, 3, width, height);
        let areas = ui::workspace_areas(screen, Focus::Library, &workspace);
        assert!(areas.library.is_none());
        assert!(areas.articles.is_none());
        assert_eq!(areas.reader, Some(screen));
        let content = ui::workspace_reader_content(screen, &workspace);
        let expected = 80.min(width - 4);
        assert_eq!(content.width, expected);
        assert_eq!(content.height, height - 8);
        assert_eq!(content.x, screen.x + (width - expected) / 2);
    }
    assert_eq!(workspace.reading_width, 72);
}

#[test]
fn fullscreen_draws_article_and_exit_hint_without_application_chrome()
-> Result<()> {
    let (_directory, store, _feed) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    text(&mut app, "Z")?;
    let title = app
        .current()
        .ok_or_else(|| anyhow!("Missing article"))?
        .title
        .clone();
    for (width, height) in [(24, 10), (80, 24), (144, 44)] {
        let mut terminal = Terminal::new(TestBackend::new(width, height))?;
        let column = ui::workspace_reader_content(
            Rect::new(0, 0, width, height),
            &app.workspace,
        );
        app.resize_reader(column.width.into(), column.height.into());
        terminal.draw(|frame| ui::draw(frame, &app))?;
        let buffer = terminal.backend().buffer();
        assert_eq!(buffer[(0, 0)].symbol(), "╭");
        let rendered = buffer
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(!rendered.contains("NORMAL"));
        assert!(!rendered.contains("01  Library"));
        if width == 144 {
            assert!(rendered.contains(&title));
            assert!(rendered.contains("Esc / F11 exit"));
        }
    }
    Ok(())
}

#[test]
fn closing_overlays_or_visual_selection_does_not_exit_fullscreen() -> Result<()>
{
    let (_directory, store, _feed) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    text(&mut app, "Z?")?;
    press(&mut app, KeyCode::Esc)?;
    assert!(app.workspace.fullscreen);
    text(&mut app, ":fs")?;
    press(&mut app, KeyCode::Esc)?;
    assert!(app.workspace.fullscreen);
    text(&mut app, "v")?;
    press(&mut app, KeyCode::Esc)?;
    assert!(app.selection.is_none());
    assert!(app.workspace.fullscreen);
    press(&mut app, KeyCode::Esc)?;
    assert!(!app.workspace.fullscreen);
    Ok(())
}

#[test]
fn fullscreen_command_alias_and_restart_restore_reader_focus() -> Result<()> {
    let (directory, store, _feed) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    text(&mut app, ":fs")?;
    press(&mut app, KeyCode::Enter)?;
    assert!(app.workspace.fullscreen);
    drop(app);
    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    let mut restored = App::new(store, false)?;
    assert!(restored.workspace.fullscreen);
    assert_eq!(restored.focus, Focus::Reader);
    text(&mut restored, "Z")?;
    assert!(!restored.workspace.fullscreen);
    assert!(matches!(restored.mode, Mode::Normal));
    let legacy: Workspace = serde_json::from_str(r#"{ "zen": true }"#)?;
    assert!(!legacy.fullscreen);
    assert!(legacy.zen);
    Ok(())
}

#[test]
fn empty_library_rejects_fullscreen_and_navigation_can_restore_panels()
-> Result<()> {
    let directory = tempfile::tempdir()?;
    let store = Store::open(&directory.path().join("empty.sqlite3"))?;
    let mut empty = App::new(store, false)?;
    assert!(press(&mut empty, KeyCode::F(11)).is_err());
    assert!(!empty.workspace.fullscreen);
    let (_directory, store, _feed) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    text(&mut app, "Z")?;
    press(&mut app, KeyCode::Tab)?;
    assert!(!app.workspace.fullscreen);
    assert_eq!(app.focus, Focus::Library);
    Ok(())
}

#[test]
fn reflow_keeps_current_passage_visible_and_preserves_zen_layout() -> Result<()>
{
    let (_directory, mut store, feed) = common::seeded_store()?;
    let body = "Replicated logs preserve the order of committed operations. "
        .repeat(400);
    store.ingest(
        &feed,
        &feed.title,
        &[lume::model::IncomingArticle {
            body_origin: lume::model::FeedOrigin::Feed,
            external_id: "long-reading-session".into(),
            title: "Reasoning about replicated logs".into(),
            url: String::new(),
            author: String::new(),
            published: 2_000_000_000,
            body,
        }],
    )?;
    let mut app = App::new(store, false)?;
    text(&mut app, "z")?;
    app.resize_reader(50, 5);
    text(&mut app, "jjjjjjjjjj")?;
    let anchor = app.line_ranges[app.scroll].start;
    assert!(anchor > 0);

    text(&mut app, "Z")?;
    let column =
        ui::workspace_reader_content(Rect::new(0, 0, 144, 44), &app.workspace);
    app.resize_reader(column.width.into(), column.height.into());

    assert!(app.line_ranges[app.scroll].contains(&anchor));
    text(&mut app, " z")?;
    assert!(!app.workspace.fullscreen);
    assert!(app.workspace.zen);
    assert_eq!(app.workspace.reading_width, 82);
    Ok(())
}
