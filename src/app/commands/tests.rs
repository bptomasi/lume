//! Registry coverage calls application handlers without executing returned I/O.

use super::*;
use crate::{command::command_suggestions, store::Store};

fn isolated_app() -> Result<(tempfile::TempDir, App)> {
    let directory = tempfile::tempdir()?;
    let mut store = Store::open(&directory.path().join("library.sqlite3"))?;
    store.subscribe("https://journal.example/feed")?;
    let feed = store
        .feeds()?
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("Missing fixture feed"))?;
    let parsed = crate::source::parse(
        include_bytes!("../../../tests/fixtures/rss.xml"),
        &feed.url,
    )?;
    store.ingest(&feed, &parsed.title, &parsed.articles)?;
    let mut app = App::new(store, false)?;
    app.set_network_enabled(false);
    Ok((directory, app))
}

fn coverage_argument(command: Command, directory: &std::path::Path) -> String {
    match command {
        Command::Export | Command::ExportNotebook | Command::ExportOpml => {
            directory.join("export").to_string_lossy().into_owned()
        }
        _ => String::new(),
    }
}

#[test]
fn every_registered_spelling_reaches_an_explicit_handler() -> Result<()> {
    for spec in command_suggestions("") {
        let (directory, mut app) = isolated_app()?;
        let argument = coverage_argument(spec.command, directory.path());
        let expected = app
            .dispatch_command(spec.command, &argument)
            .map_err(|error| format!("{error:#}"));
        if let Err(error) = &expected {
            assert!(
                !error.contains("Unknown command"),
                "{}: {error}",
                spec.name
            );
        }
        for spelling in
            std::iter::once(spec.name).chain(spec.aliases.iter().copied())
        {
            let (directory, mut app) = isolated_app()?;
            let argument = coverage_argument(spec.command, directory.path());
            // Compare complete outcomes against the unguarded typed handler.
            // A guard cannot stand in for reaching an alias's actual owner,
            // and a real handler error must remain an error. Returned I/O
            // actions stay inert; only the temporary fixture can change.
            let actual = app
                .command(&format!("{spelling} {argument}"))
                .map_err(|error| format!("{error:#}"));
            assert_eq!(actual, expected, "{spelling}");
        }
    }
    Ok(())
}

#[test]
fn note_metadata_commands_resolve_targets_before_validating_arguments()
-> Result<()> {
    let (_directory, mut app) = isolated_app()?;
    for text in ["note-type invalid-kind", "note-status invalid-status"] {
        let error = app.command(text).err().ok_or_else(|| {
            anyhow::anyhow!("Metadata command admitted a missing note")
        })?;
        assert_eq!(error.to_string(), "Select a note first");
    }
    let article = app
        .current()
        .ok_or_else(|| anyhow::anyhow!("Missing fixture article"))?
        .id;
    app.store.save_comment(article, None, "Fixture note")?;
    app.load_current()?;
    let error =
        app.command("note-status invalid-status")
            .err()
            .ok_or_else(|| {
                anyhow::anyhow!("Metadata command admitted an invalid status")
            })?;
    assert_eq!(error.to_string(), "Use open or resolved");
    Ok(())
}

#[test]
fn aliases_share_read_only_linked_page_policies() -> Result<()> {
    let (_directory, mut app) = isolated_app()?;
    app.web.active = true;
    for spec in command_suggestions("") {
        let session =
            matches!(command_owner(spec.command), CommandOwner::Session(_));
        for spelling in
            std::iter::once(spec.name).chain(spec.aliases.iter().copied())
        {
            let result = app.command(spelling);
            if !session && !spec.command.allowed_on_linked_page() {
                let error = result.err().ok_or_else(|| {
                    anyhow::anyhow!("Read-only linked page admitted {spelling}")
                })?;
                assert!(
                    error.to_string().contains("Linked pages are read-only")
                );
            }
        }
    }
    Ok(())
}

#[test]
fn command_arguments_preserve_unicode_and_reject_session_arguments()
-> Result<()> {
    assert_eq!(
        command_parts("  list Ciência e dados  "),
        ("list", "Ciência e dados")
    );
    let (_directory, mut app) = isolated_app()?;
    for spec in command_suggestions("") {
        if !matches!(command_owner(spec.command), CommandOwner::Session(_)) {
            continue;
        }
        for spelling in
            std::iter::once(spec.name).chain(spec.aliases.iter().copied())
        {
            assert!(
                app.command(&format!("{spelling} extra")).is_err(),
                "{spelling}"
            );
        }
    }
    Ok(())
}

#[test]
fn stale_reader_rejects_article_actions_and_aliases_before_mutation()
-> Result<()> {
    let (_directory, mut app) = isolated_app()?;
    let before = app.current().cloned();
    app.navigation_loading = true;
    for text in [
        "mark-read",
        "favorite",
        "delete",
        "delete-post",
        "restore",
        "list New folder",
        "image",
        "note",
        "panel reader",
        "width 80",
    ] {
        let error = app
            .command(text)
            .err()
            .ok_or_else(|| anyhow::anyhow!("Stale reader admitted {text}"))?;
        assert!(
            error
                .to_string()
                .contains("previous source is still displayed")
        );
    }
    for command in [Command::Fullscreen, Command::PanelFocus, Command::Refresh]
    {
        app.guard_stale_command(command)?;
    }
    let article =
        before.ok_or_else(|| anyhow::anyhow!("Missing fixture article"))?;
    let stored = app.store.article(article.id)?;
    assert_eq!(stored.unread, article.unread);
    assert_eq!(stored.starred, article.starred);
    assert!(!app.store.is_deleted(article.id)?);
    assert!(app.store.catalog()?.collections.is_empty());
    Ok(())
}
