//! Note completions cannot combine a selected article with retained source rows.

use anyhow::{Context, Result};

use crate::{app::App, model::IncomingArticle, store::Store};

fn application() -> Result<(tempfile::TempDir, App)> {
    let directory = tempfile::tempdir()?;
    let mut store = Store::open(&directory.path().join("library.sqlite3"))?;
    store.subscribe("https://journal.example/feed")?;
    let feed = store.feeds()?.remove(0);
    for index in 0..2 {
        store.ingest(
            &feed,
            "Journal",
            &[IncomingArticle {
                external_id: index.to_string(),
                title: format!("Article {index}"),
                url: format!("https://journal.example/{index}"),
                author: "Fixture".into(),
                published: 1_800_000_000 + index,
                body: format!("Publisher text for article {index}."),
                body_origin: crate::model::FeedOrigin::Feed,
            }],
        )?;
    }
    for article in store.library()?.articles {
        store.save_comment(
            article.id,
            None,
            &format!("Note {}", article.id),
        )?;
    }
    Ok((directory, App::new(store, false)?))
}

#[test]
fn note_completions_preserve_retained_rows_until_matching_reading_data_arrives()
-> Result<()> {
    let (_directory, mut app) = application()?;
    let before = app.reader_text.clone();
    let previous_note = app.comments[0].id;
    app.selected = 1;
    app.navigation_loading = true;
    let article = app.current().context("Selected article")?.id;
    app.store.save_comment(article, None, "New target note")?;
    let comments = app.store.comments(article)?;

    app.install_note_changes(article, comments);

    assert_eq!(app.reader_text, before);
    assert_eq!(app.comments[0].id, previous_note);
    assert_eq!(app.current().context("Selection")?.comments, 2);
    app.load_current()?;
    assert!(!app.reader_stale());
    assert!(app.reader_text.contains("New target note"));
    assert!(!app.comments.iter().any(|note| note.id == previous_note));
    Ok(())
}
