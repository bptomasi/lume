//! Ex suggestions and folder workflows exercise actual keys and persistence.

mod common;

use anyhow::{Result, anyhow};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lume::{
    app::{Action, App, Mode, PickerKind, command_suggestions},
    model::View,
    store::Store,
};

fn press(app: &mut App, code: KeyCode) -> Result<Action> {
    app.key(KeyEvent::new(code, KeyModifiers::NONE))
}

fn type_text(app: &mut App, text: &str) -> Result<()> {
    for character in text.chars() {
        press(app, KeyCode::Char(character))?;
    }
    Ok(())
}

fn run(app: &mut App, command: &str) -> Result<()> {
    type_text(app, &format!(":{command}"))?;
    press(app, KeyCode::Enter)?;
    Ok(())
}

#[test]
fn colon_suggests_filters_and_completes_without_executing() -> Result<()> {
    let (_directory, store, _feed) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    type_text(&mut app, ":fold")?;
    assert_eq!(command_suggestions("fold").len(), 2);
    press(&mut app, KeyCode::Down)?;
    press(&mut app, KeyCode::Tab)?;
    let Mode::Input(input) = &app.mode else {
        return Err(anyhow!("Completion closed the command editor"));
    };
    assert_eq!(input.text, "folders");

    press(&mut app, KeyCode::Enter)?;

    assert!(matches!(
        &app.mode,
        Mode::Picker(picker) if matches!(picker.kind, PickerKind::Folders)
    ));
    Ok(())
}

#[test]
fn incomplete_commands_require_an_argument_and_unknown_commands_keep_draft()
-> Result<()> {
    let (_directory, store, _feed) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    type_text(&mut app, ":fold")?;
    press(&mut app, KeyCode::Enter)?;
    press(&mut app, KeyCode::Enter)?;
    assert!(app.status.contains("folder NAME"));
    assert!(app.library.collections.is_empty());
    type_text(&mut app, "Redes")?;
    press(&mut app, KeyCode::Enter)?;
    assert_eq!(app.library.collections[0].name, "Redes");
    type_text(&mut app, ":not-a-command")?;
    let error = press(&mut app, KeyCode::Enter)
        .err()
        .ok_or_else(|| anyhow!("Unknown command unexpectedly succeeded"))?;
    assert!(error.to_string().contains("Unknown command"));
    let Mode::Input(input) = &app.mode else {
        return Err(anyhow!("Invalid command discarded the draft"));
    };
    assert_eq!(input.text, "not-a-command");
    Ok(())
}

#[test]
fn folder_commands_preserve_articles_favorites_and_reversible_memberships()
-> Result<()> {
    let (directory, store, _feed) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    let article = app.current().ok_or_else(|| anyhow!("Missing article"))?.id;
    run(&mut app, "save Sistemas distribuídos")?;
    run(&mut app, "save Kubernetes")?;
    run(&mut app, "favorite")?;
    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    let lists = store.catalog()?.collections;
    assert_eq!(store.article_memberships(article)?.len(), 2);
    assert!(store.article(article)?.starred);
    app.set_view(View::Collection(lists[0].id))?;
    run(&mut app, "rename Estudar com calma")?;
    run(&mut app, "delete-folder")?;
    assert_eq!(store.catalog()?.collections.len(), 1);
    assert!(store.article(article)?.starred);
    run(&mut app, "undo")?;
    assert_eq!(store.article_memberships(article)?.len(), 2);
    assert!(
        store
            .catalog()?
            .collections
            .iter()
            .any(|list| list.name == "Estudar com calma")
    );
    Ok(())
}

#[test]
fn folders_can_be_created_and_opened_without_cached_articles() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let store = Store::open(&directory.path().join("empty.sqlite3"))?;
    let mut app = App::new(store, false)?;
    run(&mut app, "folders")?;
    press(&mut app, KeyCode::Enter)?;
    type_text(&mut app, "Infraestrutura")?;
    press(&mut app, KeyCode::Enter)?;
    run(&mut app, "folders")?;
    type_text(&mut app, "Infraestrutura")?;
    press(&mut app, KeyCode::Enter)?;
    assert!(matches!(app.view, View::Collection(_)));
    assert_eq!(app.page_total, 0);
    type_text(&mut app, ":save Sem artigo")?;
    let error = press(&mut app, KeyCode::Enter)
        .err()
        .ok_or_else(|| anyhow!("Saving without an article succeeded"))?;
    assert!(error.to_string().contains("Select an article"));
    assert_eq!(app.library.collections.len(), 1);
    Ok(())
}

#[test]
fn completion_preserves_unicode_arguments_and_legacy_aliases() -> Result<()> {
    let (_directory, store, _feed) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    type_text(&mut app, ":list Ciência e dados")?;
    press(&mut app, KeyCode::Tab)?;
    let Mode::Input(input) = &app.mode else {
        return Err(anyhow!("Missing command draft"));
    };
    assert_eq!(input.text, "folder Ciência e dados");
    press(&mut app, KeyCode::Enter)?;
    assert_eq!(app.library.collections[0].name, "Ciência e dados");
    run(&mut app, "favorites")?;
    assert_eq!(app.view, View::Starred);
    run(&mut app, "help")?;
    assert!(matches!(app.mode, Mode::Help));
    Ok(())
}
