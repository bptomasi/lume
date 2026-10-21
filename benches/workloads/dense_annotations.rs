//! Revised sources with thousands of repeated, ambiguous and missing passages.

use std::path::Path;

use anyhow::{Context, Result, ensure};
use lume::{
    app::{AnnotationPosition, App},
    model::{ArticleTarget, ExtractedDocument},
    source,
    store::Store,
};

pub fn run(iterations: usize, stress: bool) -> Result<()> {
    let directory = tempfile::tempdir()?;
    let database = directory.path().join("dense.sqlite3");
    let notes = if stress { 1800 } else { 90 };
    let store = fixture(&database, notes, stress)?;
    let expected = store.comments(1)?.len();
    println!("revised source: {notes} notes, {expected} retained annotations");
    super::measure(
        "revised source + repeated quote relocation + 80-column marks",
        iterations,
        || {
            let mut app = App::new(Store::open(&database)?, false)?;
            app.resize_reader(80, 32);
            let located = app
                .annotations
                .iter()
                .filter(|entry| {
                    matches!(entry.position, AnnotationPosition::Located { .. })
                })
                .count();
            let unplaced = app
                .annotations
                .iter()
                .filter(|entry| {
                    matches!(entry.position, AnnotationPosition::Unplaced)
                })
                .count();
            ensure!(
                located == notes / 3 && unplaced == notes * 2 / 3,
                "Ambiguous or missing quotation was incorrectly relocated"
            );
            ensure!(app.content_revision == 2, "Wrong cached article revision");
            ensure!(
                app.comments.len() == expected,
                "Rendering lost annotations"
            );
            Ok(())
        },
    )
}

fn fixture(path: &Path, notes: usize, stress: bool) -> Result<Store> {
    let mut store = Store::open(path)?;
    let id = store.subscribe("https://fixture.example/feed")?;
    let feed = store
        .feeds()?
        .into_iter()
        .find(|feed| feed.id == id)
        .context("Feed")?;
    let parsed = source::parse(
        include_bytes!("../../tests/fixtures/rss.xml"),
        &feed.url,
    )?;
    store.ingest(&feed, &parsed.title, &parsed.articles)?;
    let article = store.library()?.articles[0].clone();
    let target = ArticleTarget {
        article: article.id,
        url: article.url,
    };
    let paragraphs = if stress { 4000 } else { 400 };
    let body = format!(
        "# Revised source\n\nUnique stable quotation\n\nAmbiguous quotation\n\nAmbiguous quotation\n\n{}",
        "Durable state preserves the admitted intention.\n\n"
            .repeat(paragraphs)
    );
    for body in ["Earlier publisher revision".to_owned(), body] {
        store.cache_document(
            &target,
            &ExtractedDocument {
                final_url: target.url.clone(),
                body,
            },
        )?;
    }
    let connection = rusqlite::Connection::open(path)?;
    connection.execute(
        include_str!("../../src/store/sql/test_insert_dense_annotations.sql"),
        (article.id, notes),
    )?;
    Ok(store)
}
