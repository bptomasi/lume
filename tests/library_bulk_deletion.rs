//! Sidebar views delete their entire matching set; definitions keep articles.
mod common;

use anyhow::{Context, Result};
use crossterm::event::KeyCode;
use lume::{
    app::{App, Focus},
    model::{ArticleQuery, IncomingArticle, View},
    store::Store,
};

fn fixture(count: usize) -> Result<(tempfile::TempDir, Store)> {
    let (directory, mut store, feed) = common::seeded_store()?;
    let articles = (0..count)
        .map(|id| IncomingArticle {
            external_id: format!("bulk:{id}"),
            title: format!("Article {id}"),
            url: format!("https://journal.example/bulk/{id}"),
            author: String::new(),
            published: 2_000_000_000 + id as i64,
            body: "Preserve this source text".into(),
            body_origin: lume::model::FeedOrigin::Feed,
        })
        .collect::<Vec<_>>();
    store.ingest(&feed, &feed.title, &articles)?;
    rusqlite::Connection::open(directory.path().join("library.sqlite3"))?
        .execute_batch(include_str!(
            "../src/store/sql/deletion/test_bulk_flags.sql"
        ))?;
    store.clear_history()?;
    Ok((directory, store))
}

fn matching(store: &Store, view: View) -> Result<Vec<i64>> {
    Ok(store
        .query_articles(&ArticleQuery {
            view,
            text: String::new(),
            offset: 0,
            limit: 500,
            retained: None,
        })?
        .articles
        .iter()
        .map(|article| article.id)
        .collect())
}

fn dd(app: &mut App, view: View) -> Result<()> {
    app.focus = Focus::Library;
    app.nav_selected = app
        .navigation
        .iter()
        .position(|item| item.view == view)
        .context("Missing Library destination")?;
    app.key(KeyCode::Char('d').into())?;
    app.key(KeyCode::Char('d').into())?;
    Ok(())
}

#[test]
fn every_builtin_scope_moves_its_articles_and_undo_preserves_personal_state()
-> Result<()> {
    for view in [
        View::Inbox,
        View::All,
        View::Queue,
        View::Starred,
        View::Read,
        View::Annotated,
        View::Continue,
        View::SinceLastVisit(150),
        View::Recommended,
    ] {
        let (_directory, store) = fixture(20)?;
        let before = store.library()?.articles;
        let mut expected = matching(&store, view.clone())?;
        assert!(!expected.is_empty(), "Empty scope fixture: {view:?}");
        assert_eq!(store.trash_view(&view)?, expected.len());
        let mut removed = matching(&store, View::Trash)?;
        expected.sort_unstable();
        removed.sort_unstable();
        assert_eq!(removed, expected, "Wrong deleted set: {view:?}");
        assert!(store.undo()?);
        let restored = store.library()?.articles;
        for (old, new) in before.iter().zip(&restored) {
            assert_eq!(
                (old.id, old.unread, old.starred, old.queued),
                (new.id, new.unread, new.starred, new.queued)
            );
        }
        assert!(!store.undo()?);
    }
    Ok(())
}

#[test]
fn whole_view_deletion_has_no_page_limit_and_grouped_includes_duplicates()
-> Result<()> {
    for view in [View::All, View::Grouped] {
        let (_directory, store) = fixture(650)?;
        assert_eq!(store.trash_view(&view)?, 651);
        assert_eq!(store.view_count(&View::All)?, 0);
        assert_eq!(store.view_count(&View::Trash)?, 651);
        assert!(store.undo()?);
        assert_eq!(store.view_count(&View::All)?, 651);
    }
    Ok(())
}

#[test]
fn purging_part_of_a_bulk_action_preserves_undo_for_remaining_articles()
-> Result<()> {
    let (_directory, store) = fixture(20)?;
    let target = store.library()?.articles[0].id;
    store.trash_view(&View::All)?;
    store.purge_deleted(target)?;
    assert!(store.undo()?);
    assert_eq!(store.view_count(&View::All)?, 20);
    assert_eq!(store.view_count(&View::Trash)?, 0);
    assert!(!store.undo()?);
    assert!(store.article(target).is_err());
    Ok(())
}

#[test]
fn failed_bulk_action_rolls_back_every_article_and_creates_no_history()
-> Result<()> {
    let (directory, store) = fixture(20)?;
    rusqlite::Connection::open(directory.path().join("library.sqlite3"))?
        .execute_batch(include_str!(
            "../src/store/sql/deletion/test_reject_bulk.sql"
        ))?;
    assert!(store.trash_view(&View::All).is_err());
    assert_eq!(store.view_count(&View::All)?, 21);
    assert_eq!(store.view_count(&View::Trash)?, 0);
    assert!(!store.undo()?);
    Ok(())
}

#[test]
fn sidebar_dd_trashes_a_view_and_trash_dd_permanently_empties_it() -> Result<()>
{
    let (directory, store) = fixture(20)?;
    let mut app = App::new(store, false)?;
    app.page_size = 1;
    app.set_view(View::All)?;
    dd(&mut app, View::All)?;
    let observer = Store::open(&directory.path().join("library.sqlite3"))?;
    assert_eq!(observer.view_count(&View::Trash)?, 21);
    app.set_view(View::Trash)?;
    dd(&mut app, View::Trash)?;
    assert_eq!(observer.view_count(&View::Trash)?, 0);
    assert!(observer.library()?.articles.is_empty());
    assert!(app.current().is_none());
    assert!(!observer.undo()?);
    Ok(())
}

#[test]
fn smart_list_and_folder_deletion_keep_every_matching_article_and_note()
-> Result<()> {
    let (directory, store) = fixture(20)?;
    let target = store.library()?.articles[0].id;
    store.save_comment(target, None, "Keep this note")?;
    store.save_smart_list("Everything", &Default::default())?;
    let smart = store.smart_lists()?.remove(0).id;
    let folder = store.save_collection("Personal", Some(target))?;
    let mut app = App::new(store, false)?;
    for view in [View::Smart(smart), View::Collection(folder)] {
        dd(&mut app, view)?;
        let observer = Store::open(&directory.path().join("library.sqlite3"))?;
        assert_eq!(observer.view_count(&View::All)?, 21);
        assert_eq!(observer.view_count(&View::Trash)?, 0);
        assert_eq!(observer.comments(target)?.len(), 1);
    }
    assert!(app.library.smart_lists.is_empty());
    assert!(app.library.collections.is_empty());
    app.key(KeyCode::Char('u').into())?;
    assert_eq!(app.library.collections.len(), 1);
    Ok(())
}

#[test]
fn background_sidebar_action_keeps_its_captured_scope_after_navigation_changes()
-> Result<()> {
    let (directory, store) = fixture(20)?;
    let expected = store.view_count(&View::Starred)?;
    let path = directory.path().join("library.sqlite3");
    let mut app = App::new(store, false)?;
    app.enable_background_navigation(&path)?;
    dd(&mut app, View::Starred)?;
    app.set_view(View::Read)?;
    common::settle_notes(&mut app)?;
    let observer = Store::open(&path)?;
    assert_eq!(observer.view_count(&View::Trash)?, expected);
    assert_eq!(observer.view_count(&View::Starred)?, 0);
    app.end_session()
}

#[test]
fn purging_a_member_keeps_folder_undo_without_resurrecting_the_member()
-> Result<()> {
    let (_directory, store) = fixture(20)?;
    let folder = store.save_collection("Original", Some(1))?;
    store.clear_history()?;
    store.rename_collection(folder, "Renamed")?;
    store.set_deleted(1, true)?;
    store.purge_deleted(1)?;
    assert!(store.undo()?);
    let collection = store.library()?.collections.remove(0);
    assert_eq!(collection.name, "Original");
    assert!(collection.articles.is_empty());
    assert!(store.article(1).is_err());
    assert!(!store.undo()?);
    Ok(())
}

#[test]
fn purging_a_related_article_keeps_note_edit_undo_with_pruned_links()
-> Result<()> {
    let (_directory, store) = fixture(20)?;
    let note = store.save_comment(1, None, "Original note")?;
    store.link_annotation(1, note, 2)?;
    store.clear_history()?;
    store.save_comment(1, Some(note), "Changed note")?;
    store.set_deleted(2, true)?;
    store.purge_deleted(2)?;
    assert!(store.undo()?);
    let restored = store.annotation(1, note)?;
    assert_eq!(restored.comment.body, "Original note");
    assert!(restored.links.is_empty());
    assert!(store.article(2).is_err());
    assert!(!store.undo()?);
    Ok(())
}

#[test]
fn purging_one_owner_preserves_surviving_edits_in_a_nested_undo_batch()
-> Result<()> {
    let (_directory, store) = fixture(20)?;
    let ids = [1, 2, 3].into();
    store.edit_articles(&ids, &lume::model::BulkEdit::Favorite(true))?;
    store.set_deleted(1, true)?;
    store.purge_deleted(1)?;
    assert!(store.undo()?, "The surviving batch must remain reversible");
    assert!(!store.article(2)?.starred);
    assert!(store.article(3)?.starred);
    assert!(store.article(1).is_err());
    assert!(!store.undo()?);
    Ok(())
}
