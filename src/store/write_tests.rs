//! Write authority precedes captured reads; no-op edits preserve real undo work.
use super::*;
use crate::model::{ArticleTarget, BulkEdit, ExtractedDocument, TagTarget};
use rusqlite::hooks::{AuthAction, AuthContext, Authorization};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU8, Ordering},
};

struct Fixture {
    directory: tempfile::TempDir,
    store: Store,
}
impl Fixture {
    fn new() -> Result<Self> {
        let directory = tempfile::tempdir()?;
        let mut store = Store::open(&directory.path().join("library.sqlite3"))?;
        store.subscribe("https://journal.example/feed")?;
        let feed = store.feeds()?.remove(0);
        let parsed = crate::source::parse(
            include_bytes!("../../tests/fixtures/rss.xml"),
            &feed.url,
        )?;
        store.ingest(&feed, &parsed.title, &parsed.articles)?;
        store.save_comment(1, None, "Original note")?;
        store.clear_history()?;
        Ok(Self { directory, store })
    }

    fn document(&self) -> Result<(ArticleTarget, ExtractedDocument)> {
        let target = ArticleTarget {
            article: 1,
            url: self.store.article(1)?.url,
        };
        let document = ExtractedDocument {
            final_url: target.url.clone(),
            body: "Independent readable full content. ".repeat(1000),
        };
        Ok((target, document))
    }

    fn competing_writer(&self) -> Result<Arc<AtomicU8>> {
        let rival =
            Connection::open(self.directory.path().join("library.sqlite3"))?;
        rival.busy_timeout(Duration::ZERO)?;
        let rival = Mutex::new(rival);
        let observed = Arc::new(AtomicU8::new(0));
        let result = observed.clone();
        self.store.connection.authorizer(Some(
            move |context: AuthContext<'_>| {
                if result.load(Ordering::Acquire) == 0
                    && matches!(
                        context.action,
                        AuthAction::Insert { .. }
                            | AuthAction::Update { .. }
                            | AuthAction::Delete { .. }
                    )
                {
                    let status = match rival.lock() {
                        Ok(rival) => match rival.execute(
                            include_str!("sql/test_concurrent_feed_health.sql"),
                            [],
                        ) {
                            Ok(_) => 1,
                            Err(rusqlite::Error::SqliteFailure(error, _))
                                if error.code
                                    == rusqlite::ErrorCode::DatabaseBusy =>
                            {
                                2
                            }
                            Err(_) => 3,
                        },
                        Err(_) => 3,
                    };
                    result.store(status, Ordering::Release);
                }
                Authorization::Allow
            },
        ));
        Ok(observed)
    }
}

#[test]
fn personal_writes_exclude_competing_commits_before_their_first_change()
-> Result<()> {
    for operation in 0..7 {
        let fixture = Fixture::new()?;
        let store = &fixture.store;
        if operation == 5 {
            store.set_starred(1, true)?;
        }
        let observed = fixture.competing_writer()?;
        match operation {
            0 => store.set_tags(TagTarget::Article(1), &["new".into()])?,
            1 => {
                store.save_comment(1, Some(1), "Revised note")?;
            }
            2 => store.edit_articles(&[1].into(), &BulkEdit::Favorite(true))?,
            3 => {
                let (target, document) = fixture.document()?;
                store.cache_document(&target, &document)?;
            }
            4 => {
                store.set_deleted(1, true)?;
            }
            5 => {
                assert!(store.undo()?);
            }
            _ => {
                store.refresh_recommendations()?;
            }
        }
        assert_eq!(
            observed.load(Ordering::Acquire),
            2,
            "operation {operation}"
        );
        store
            .connection
            .authorizer(None::<fn(AuthContext<'_>) -> Authorization>);
    }
    Ok(())
}

#[test]
fn unchanged_bulk_flags_and_tags_do_not_hide_the_last_real_edit() -> Result<()>
{
    let fixture = Fixture::new()?;
    let store = &fixture.store;
    store.set_starred(1, true)?;
    let changes = store.connection.total_changes();
    store.edit_articles(&[1].into(), &BulkEdit::Unread(true))?;
    store.set_tags(TagTarget::Article(1), &[])?;
    assert_eq!(store.connection.total_changes(), changes);
    assert!(store.undo()?);
    assert!(!store.article(1)?.starred);
    assert!(!store.undo()?);
    assert!(store.set_tags(TagTarget::Article(9999), &[]).is_err());
    Ok(())
}

#[test]
fn tags_change_only_their_difference_and_existing_full_capacity_tags_are_idempotent()
-> Result<()> {
    let fixture = Fixture::new()?;
    let store = &fixture.store;
    store.set_tags(TagTarget::Article(1), &["keep".into(), "old".into()])?;
    store
        .connection
        .execute_batch(include_str!("sql/test_keep_article_tag.sql"))?;
    store.set_tags(TagTarget::Article(1), &["keep".into(), "new".into()])?;
    assert!(store.undo()?);
    assert_eq!(store.tags(TagTarget::Article(1))?, ["keep", "old"]);
    let tags = (0..64).map(|id| format!("tag{id:02}")).collect::<Vec<_>>();
    store.set_tags(TagTarget::Comment(1), &tags)?;
    let changes = store.connection.total_changes();
    store.set_tags(TagTarget::Comment(1), &tags)?;
    assert_eq!(store.connection.total_changes(), changes);
    store
        .connection
        .execute_batch(include_str!("sql/test_drop_keep_article_tag.sql"))?;
    store.set_tags(TagTarget::Article(1), &tags)?;
    let changes = store.connection.total_changes();
    store.edit_articles(&[1].into(), &BulkEdit::Tag(" tag00 ".into()))?;
    assert_eq!(store.connection.total_changes(), changes);
    assert!(
        store
            .edit_articles(&[1].into(), &BulkEdit::Tag("overflow".into()))
            .is_err()
    );
    assert_eq!(store.tags(TagTarget::Article(1))?, tags);
    Ok(())
}

#[test]
fn unchanged_note_saves_do_not_consume_undo_or_rewrite_versions() -> Result<()>
{
    let fixture = Fixture::new()?;
    let store = &fixture.store;
    store.set_starred(1, true)?;
    let changes = store.connection.total_changes();
    store.save_comment_checked(
        1,
        Some(1),
        "Original note",
        Some("Original note"),
    )?;
    assert_eq!(store.connection.total_changes(), changes);
    assert!(store.undo()?);
    assert!(!store.article(1)?.starred);
    assert!(
        store
            .save_comment_checked(
                1,
                Some(1),
                "Original note",
                Some("stale baseline")
            )
            .is_err()
    );
    Ok(())
}

#[test]
fn bulk_flags_validate_all_owners_and_trash_before_writing() -> Result<()> {
    let fixture = Fixture::new()?;
    let store = &fixture.store;
    for edit in [
        BulkEdit::Favorite(true),
        BulkEdit::Unread(false),
        BulkEdit::Queue(true),
    ] {
        assert!(store.edit_articles(&[1, 9999].into(), &edit).is_err());
        assert!(!store.article(1)?.starred);
        assert!(store.article(1)?.unread);
        assert!(!store.article(1)?.queued);
    }
    store.set_deleted(1, true)?;
    assert!(
        store
            .edit_articles(&[1].into(), &BulkEdit::Favorite(true))
            .is_err()
    );
    Ok(())
}

#[test]
fn unchanged_full_cache_preserves_revision_progress_and_avoids_fts_rewrites()
-> Result<()> {
    let fixture = Fixture::new()?;
    let store = &fixture.store;
    let (target, body) = fixture.document()?;
    store.cache_document(&target, &body)?;
    store.save_document_progress(1, 1, 24)?;
    let changes = store.connection.total_changes();
    store.cache_document(&target, &body)?;
    assert_eq!(store.connection.total_changes() - changes, 1);
    let full = store.cached_document(1)?.context("Full cache")?;
    assert_eq!((full.revision, full.progress), (1, 24));
    let replacement = ExtractedDocument {
        body: "Different zenithword full content.".into(),
        ..body
    };
    store.cache_document(&target, &replacement)?;
    let full = store.cached_document(1)?.context("Changed cache")?;
    assert_eq!((full.revision, full.progress), (2, 0));
    Ok(())
}

#[test]
fn idle_recommendations_remain_readable_while_another_writer_is_active()
-> Result<()> {
    let fixture = Fixture::new()?;
    while fixture.store.refresh_recommendations()?.remaining {}
    let rival = Store::open(&fixture.directory.path().join("library.sqlite3"))?;
    let transaction = rival.write_transaction()?;
    let idle = fixture.store.refresh_recommendations()?;
    assert_eq!(idle.processed, 0);
    assert!(!idle.remaining);
    transaction.rollback()?;
    Ok(())
}
