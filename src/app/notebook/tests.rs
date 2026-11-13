use super::*;

#[test]
fn acknowledged_search_history_outlives_an_older_listing_snapshot() -> Result<()>
{
    let (_directory, store) = fixture()?;
    let mut notebook =
        Notebook::start(&store.database_path()?, "", None, origin(&store)?)?;
    let previous = listing(0, Vec::new());

    notebook.install_recent(vec!["body:consensus".into()]);
    notebook.install_listing(previous);

    assert_eq!(notebook.recent, ["body:consensus"]);
    let current = listing(1, vec!["body:protocol".into()]);
    notebook.install_listing(current);
    assert_eq!(notebook.recent, ["body:protocol"]);
    Ok(())
}

fn listing(generation: u64, recent: Vec<String>) -> readers::Listing {
    readers::Listing {
        page: NotePage::default(),
        offset: 0,
        collections: Vec::new(),
        recent,
        recent_generation: generation,
    }
}

fn fixture() -> Result<(tempfile::TempDir, crate::store::Store)> {
    let directory = tempfile::tempdir()?;
    let store =
        crate::store::Store::open(&directory.path().join("library.sqlite3"))?;
    Ok((directory, store))
}

fn origin(store: &crate::store::Store) -> Result<Origin> {
    let workspace = store.workspace()?;
    let layouts = store.layouts(&workspace)?;
    Ok(Origin::capture(&workspace, &layouts, Focus::Reader))
}

fn preview(id: i64, article: i64) -> NoteSummary {
    NoteSummary {
        id,
        article,
        title: "Source title".into(),
        source: "Fixture".into(),
        body: "Note body".into(),
        preview: String::new(),
        quote: String::new(),
        created: 0,
        metadata: Default::default(),
        tags: Vec::new(),
    }
}

#[test]
fn changing_article_scope_keeps_cached_rows_but_hides_stale_action_targets()
-> Result<()> {
    use anyhow::Context;
    let (_directory, store) = fixture()?;
    let path = store.database_path()?;
    let mut state = NotebookState::default();
    state.open(&path, "", Some(10), origin(&store)?)?;
    let mut snapshot = listing(0, Vec::new());
    snapshot.page = NotePage {
        total: 1,
        notes: vec![preview(1, 10)],
    };
    state
        .active
        .as_mut()
        .context("Active notebook")?
        .install_listing(snapshot);
    assert_eq!(
        state
            .view()
            .and_then(|view| view.current())
            .map(|note| note.id),
        Some(1)
    );

    state.open(&path, "", Some(20), origin(&store)?)?;

    let view = state.view().context("Notebook remains active")?;
    assert_eq!(view.page.notes.len(), 1);
    assert!(view.current().is_none());
    assert!(view.current_context().is_none());
    assert!(view.loading);
    Ok(())
}

#[test]
fn source_return_retains_query_history_and_refreshes_the_layout_origin()
-> Result<()> {
    use anyhow::Context;
    let (_directory, store) = fixture()?;
    let first = origin(&store)?;
    let expected = first.layout.clone();
    let mut state = NotebookState::default();
    state.open(&store.database_path()?, "body:consensus", None, first)?;
    state
        .active
        .as_mut()
        .context("Active notebook")?
        .follow_reference(7)?;

    let previous = state.suspend_source().context("Source suspension")?;
    assert_eq!(previous.layout, expected);
    assert_eq!(previous.focus, Focus::Reader);
    assert!(state.view().is_none());
    let mut workspace = store.workspace()?;
    workspace.show_library = false;
    let expected = crate::model::PanelLayout::capture(&workspace);
    let layouts = store.layouts(&workspace)?;
    let next = Origin::capture(&workspace, &layouts, Focus::Reader);
    assert!(state.resume_source(next)?);
    let active = state.active.as_mut().context("Resumed notebook")?;
    assert_eq!(active.view().query, "id:7");
    active.restore_history()?;
    assert_eq!(active.view().query, "body:consensus");

    let restored = state.close().context("Origin restoration")?;
    assert_eq!(restored.layout, expected);
    assert_eq!(restored.focus, Focus::Reader);
    assert!(state.view().is_none());
    Ok(())
}

#[test]
fn suspended_editor_roundtrip_preserves_cursor_checkpoint_and_undo()
-> Result<()> {
    use crate::app::{Input, InputKind};
    use anyhow::Context;
    let mut input = Input::new(
        InputKind::Comment {
            article: 7,
            comment: Some(9),
        },
        "first\nsecond".into(),
        "prior query".into(),
    );
    input.editor.insert_str(" pending");
    input.text = input.editor.lines().join("\n");
    let cursor = input.editor.cursor();
    let mut state = NotebookState::default();

    state.suspend_editor(Box::new(input));
    assert_eq!(
        state
            .suspended_editor()
            .context("Suspended editor")?
            .saved_text,
        "first\nsecond"
    );
    let mut resumed = state.resume_editor().context("Resumed editor")?;

    assert_eq!(resumed.editor.cursor(), cursor);
    assert_eq!(resumed.text, "first\nsecond pending");
    assert_eq!(resumed.previous_search, "prior query");
    assert!(matches!(
        resumed.kind,
        InputKind::Comment {
            article: 7,
            comment: Some(9)
        }
    ));
    assert!(resumed.editor.undo());
    assert_eq!(resumed.editor.lines().join("\n"), "first\nsecond");
    assert!(state.suspended_editor().is_none());
    assert!(state.resume_editor().is_none());
    Ok(())
}
