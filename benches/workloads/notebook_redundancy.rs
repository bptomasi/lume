//! Compare discarded previews/counts and repeated stale-page recovery.

use anyhow::{Result, ensure};
use lume::{model::NotePage, store::Store};

const QUERY: &str = "body:quorum tag:consensus type:definition status:open";

pub(super) fn run(iterations: usize, stress: bool) -> Result<()> {
    let directory = tempfile::tempdir()?;
    let count = if stress { 100_000 } else { 10_000 };
    let store = super::annotation_notebook::seed(
        &directory.path().join("redundancy.sqlite3"),
        count,
        100,
    )?;
    export_pages(&store, count, iterations.min(4))?;
    stale_page(&store, count / 10, iterations)
}

fn export_pages(store: &Store, count: usize, iterations: usize) -> Result<()> {
    let reference = || {
        enumerate(|offset| {
            Ok(store
                .search_notes("", None, offset, 100)?
                .notes
                .into_iter()
                .map(|note| note.id)
                .collect())
        })
    };
    let expected = reference()?;
    ensure!(expected.len() == count, "Enumeration lost notes");
    super::measure(
        "reference export IDs through previews/counts",
        iterations,
        || {
            ensure!(reference()? == expected, "Reference order changed");
            Ok(())
        },
    )?;
    super::measure("export IDs without previews/counts", iterations, || {
        ensure!(
            enumerate(|offset| store.note_export_ids(offset, 100))? == expected,
            "Narrow enumeration changed identities or order"
        );
        Ok(())
    })
}

fn enumerate(
    mut read: impl FnMut(usize) -> Result<Vec<i64>>,
) -> Result<Vec<i64>> {
    let mut ids = Vec::new();
    loop {
        let page = read(ids.len())?;
        if page.is_empty() {
            return Ok(ids);
        }
        ids.extend(page);
    }
}

fn stale_page(store: &Store, total: usize, iterations: usize) -> Result<()> {
    let offset = (total - 1) / 100 * 100;
    let expected = store.search_notes(QUERY, None, offset, 100)?;
    ensure!(expected.total == total && expected.notes.len() == 100);
    super::measure(
        "reference stale note page with repeated search",
        iterations,
        || {
            let mut page = store.search_notes(QUERY, None, total + 100, 100)?;
            ensure!(page.notes.is_empty() && page.total == total);
            page = store.search_notes(
                QUERY,
                None,
                (page.total - 1) / 100 * 100,
                100,
            )?;
            check_page(&page, &expected)
        },
    )?;
    super::measure(
        "stale note page with one snapshot/count",
        iterations,
        || {
            let (page, actual) =
                store.search_notes_clamped(QUERY, None, total + 100, 100)?;
            ensure!(actual == offset, "Clamped offset changed");
            check_page(&page, &expected)
        },
    )
}

fn check_page(page: &NotePage, expected: &NotePage) -> Result<()> {
    ensure!(
        page.total == expected.total
            && page.notes.len() == expected.notes.len()
    );
    for (note, reference) in page.notes.iter().zip(&expected.notes) {
        ensure!(note.id == reference.id && note.preview == reference.preview);
        ensure!(note.preview.contains('\u{1e}'), "FTS evidence disappeared");
    }
    Ok(())
}
