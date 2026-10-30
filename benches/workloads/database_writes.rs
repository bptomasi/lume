//! Batched personal edits and repeated Full caches retain exact stored state.
use anyhow::{Context, Result, ensure};
use lume::{
    model::{ArticleTarget, BulkEdit, ExtractedDocument, TagTarget, View},
    store::Store,
};
use std::collections::BTreeSet;

pub(super) fn run(iterations: usize, stress: bool) -> Result<()> {
    let directory = tempfile::tempdir()?;
    let count = if stress { 10_000 } else { 2000 };
    let store = super::page_navigation::fixture(
        &directory.path().join("writes.sqlite3"),
        count,
    )?;
    let ids = (1..=1000).collect::<BTreeSet<i64>>();
    super::measure(
        "1000 favorites on + off + unchanged batch",
        iterations,
        || {
            for value in [true, false, false] {
                store.edit_articles(&ids, &BulkEdit::Favorite(value))?;
                ensure!(
                    store.view_count(&View::Starred)?
                        == if value { 1000 } else { 0 },
                    "Bulk flags changed"
                );
            }
            Ok(())
        },
    )?;
    let tags = (0..64)
        .map(|index| format!("tag{index:02}"))
        .collect::<Vec<_>>();
    store.set_tags(TagTarget::Article(1), &tags)?;
    super::measure("unchanged 64-tag replacement", iterations, || {
        store.set_tags(TagTarget::Article(1), &tags)?;
        ensure!(store.tags(TagTarget::Article(1))? == tags, "Tags changed");
        Ok(())
    })?;
    full_cache(&store, iterations)
}

fn full_cache(store: &Store, iterations: usize) -> Result<()> {
    let document = ExtractedDocument {
        final_url: "https://pages.example/final".into(),
        body: "Independent readable full content. ".repeat(1000),
    };
    let targets = (1..=8)
        .map(|article| {
            Ok(ArticleTarget {
                article,
                url: store.article(article)?.url,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    for target in &targets {
        store.cache_document(target, &document)?;
    }
    super::measure("eight unchanged Full cache commits", iterations, || {
        for target in &targets {
            ensure!(
                store.cache_document(target, &document)?,
                "Cache target disappeared"
            );
            let actual = store
                .cached_document(target.article)?
                .context("Full cache")?;
            ensure!(
                actual.body == document.body && actual.revision == 1,
                "Cache or revision drifted"
            );
        }
        Ok(())
    })
}
