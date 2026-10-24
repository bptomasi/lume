//! Stale offsets clamp in one snapshot without hydrating an impossible page.
use super::*;
use rusqlite::hooks::{AuthAction, AuthContext, Authorization};

#[test]
fn window_recovery_normalizes_limits_and_empty_or_stale_offsets() -> Result<()>
{
    let directory = tempfile::tempdir()?;
    let store = fixture(&directory.path().join("windows.sqlite3"))?;
    for limit in [0, 1, 10, 500, usize::MAX] {
        for offset in [0, 40, 70, 75, 500, usize::MAX] {
            let mut request = query(offset);
            request.limit = limit;
            let (resolved, page) = store.article_window(&request)?;
            let limit = limit.clamp(1, 500);
            let expected = if offset < 75 {
                offset
            } else {
                74 / limit * limit
            };
            assert_eq!(resolved, expected);
            assert_eq!(page.total, 75);
            assert_eq!(page.articles.len(), limit.min(75 - expected));
        }
    }
    let empty = Store::open(&directory.path().join("empty.sqlite3"))?;
    let (offset, page) = empty.article_window(&query(500))?;
    assert_eq!((offset, page.total, page.articles.len()), (0, 0, 0));
    Ok(())
}

#[test]
fn out_of_range_pages_skip_metadata_and_unchanged_totals_skip_recounting()
-> Result<()> {
    let directory = tempfile::tempdir()?;
    let store = fixture(&directory.path().join("skip.sqlite3"))?;
    store.query_articles(&query(0))?;
    store
        .connection
        .authorizer(Some(|context: AuthContext<'_>| match context.action {
            AuthAction::Read {
                table_name: "comments",
                ..
            }
            | AuthAction::Function {
                function_name: "count",
            } => Authorization::Deny,
            _ => Authorization::Allow,
        }));
    let page = store.query_articles(&query(usize::MAX))?;
    assert_eq!(page.total, 75);
    assert!(page.articles.is_empty());
    assert!(
        store.query_articles(&query(0)).is_err(),
        "Metadata guard was not active"
    );
    let mut other = query(0);
    other.text = "is:read".into();
    assert!(
        store.query_articles(&other).is_err(),
        "An uncached count was skipped"
    );
    store
        .connection
        .authorizer(None::<fn(AuthContext<'_>) -> Authorization>);
    assert_eq!(store.query_articles(&query(0))?.total, 75);
    Ok(())
}

#[test]
fn tail_recovery_keeps_the_count_snapshot_when_another_writer_deletes_articles()
-> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("coherent.sqlite3");
    let store = fixture(&path)?;
    let writer = Store::open(&path)?;
    let mut committed = false;
    store
        .connection
        .authorizer(Some(move |context: AuthContext<'_>| {
            if !committed
                && matches!(
                    context.action,
                    AuthAction::Read {
                        table_name: "comments",
                        ..
                    }
                )
            {
                committed = true;
                if writer
                    .connection
                    .execute(
                        include_str!("../../../sql/test_cancel_page_write.sql"),
                        [],
                    )
                    .is_err()
                {
                    return Authorization::Deny;
                }
            }
            Authorization::Allow
        }));
    let (offset, page) = store.article_window(&query(999))?;
    assert_eq!((offset, page.total, page.articles.len()), (70, 75, 5));
    store
        .connection
        .authorizer(None::<fn(AuthContext<'_>) -> Authorization>);
    assert_eq!(store.query_articles(&query(0))?.total, 0);
    Ok(())
}

#[test]
fn identity_jumps_ignore_stale_offsets_and_use_the_effective_page_limit()
-> Result<()> {
    let directory = tempfile::tempdir()?;
    let store = fixture(&directory.path().join("anchors.sqlite3"))?;
    for limit in [0, 7, usize::MAX] {
        let mut request = query(usize::MAX);
        request.limit = limit;
        let (offset, page) = store
            .page_containing(&request, 30)?
            .context("Anchor page")?;
        let limit = limit.clamp(1, 500);
        assert_eq!(offset, 45 / limit * limit);
        assert_eq!(page.total, 75);
        assert_eq!(page.articles[45 - offset].id, 30);
    }
    Ok(())
}
