//! Cached cursors must match cold reads for every membership and sort mode.
use super::*;
use crate::model::SmartFilter;

#[test]
fn cached_pages_match_full_metadata_across_scopes_filters_and_both_directions()
-> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("scopes.sqlite3");
    let store = fixture(&path)?;
    decorate(&store)?;
    let reference = Store::open(&path)?;
    let smart = store.smart_lists()?.remove(0).id;
    let views = [
        View::All,
        View::Inbox,
        View::Read,
        View::Starred,
        View::Queue,
        View::Annotated,
        View::Trash,
        View::Feed(1),
        View::Smart(smart),
        View::Grouped,
    ];
    for view in views {
        for text in [
            "",
            "systems",
            "tag:systems",
            "NOT tag:systems",
            "tag:systems OR is:favorite",
        ] {
            for offset in [0, 3, 6, 3, 0, 14, 17, 14, 90] {
                let mut request = query(offset);
                request.view = view.clone();
                request.text = text.into();
                request.limit = 3;
                assert_same(&store, &reference, &request)?;
            }
        }
    }
    Ok(())
}

fn decorate(store: &Store) -> Result<()> {
    store
        .connection
        .execute(include_str!("../../../sql/test_page_tags.sql"), [])?;
    for id in 1..=75 {
        if id % 2 == 0 {
            store.set_unread(id, false)?;
        }
        if id % 5 == 0 {
            store.set_starred(id, true)?;
        }
        if id % 7 == 0 {
            store.set_queued(id, true)?;
        }
        if id % 11 == 0 {
            store.set_deleted(id, true)?;
        }
    }
    store.save_comment(1, None, "Private context")?;
    store.save_smart_list(
        "Systems tags",
        &SmartFilter {
            text: "tag:systems".into(),
            ..Default::default()
        },
    )?;
    Ok(())
}

fn assert_same(
    store: &Store,
    reference: &Store,
    request: &ArticleQuery,
) -> Result<()> {
    reference.pages.borrow_mut().entries.clear();
    let expected = reference.query_articles(request)?;
    let actual = store.query_articles(request)?;
    assert_eq!(
        actual.total, expected.total,
        "Total for {:?} / {}",
        request.view, request.text
    );
    assert_eq!(
        actual.articles, expected.articles,
        "Rows for {:?} / {} / {}",
        request.view, request.text, request.offset
    );
    Ok(())
}

#[test]
fn local_and_external_membership_changes_invalidate_both_totals_and_cursors()
-> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("edits.sqlite3");
    let store = fixture(&path)?;
    let other = Store::open(&path)?;
    let mut request = query(0);
    request.view = View::Starred;
    for id in 1..=25 {
        store.set_starred(id, true)?;
    }
    assert_same(&store, &other, &request)?;
    store.set_starred(25, false)?;
    request.offset = 10;
    assert_same(&store, &other, &request)?;
    other.set_starred(75, true)?;
    request.offset = 0;
    assert_same(&store, &other, &request)?;
    let mut inbox = query(0);
    inbox.view = View::Inbox;
    inbox.retained = Some(75);
    store.set_unread(75, false)?;
    assert_same(&store, &other, &inbox)?;
    inbox.retained = None;
    assert_same(&store, &other, &inbox)?;
    Ok(())
}
