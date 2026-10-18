//! Original sample articles for an isolated, fully editable demonstration.

use anyhow::Result;

use crate::{model::IncomingArticle, store::Store};

/// Seed a separate demonstration database once; never touch real subscriptions.
///
/// # Errors
/// Returns storage errors.
pub fn seed(store: &mut Store) -> Result<()> {
    if !store.library()?.articles.is_empty() {
        return Ok(());
    }
    for (source, title, entries) in SOURCES {
        seed_source(store, source, title, entries)?;
    }
    let list = store.create_collection("Deep dives")?;
    store.create_collection("Read this weekend")?;
    if let Some(article) = store.library()?.articles.first() {
        store.add_member(list, article.id)?;
        store.set_starred(article.id, true)?;
        store.save_comment(
            article.id,
            None,
            include_str!("demo/comment.txt"),
        )?;
    }
    store.clear_history()?;
    Ok(())
}

fn seed_source(
    store: &mut Store,
    source: &str,
    title: &str,
    entries: &[Sample],
) -> Result<()> {
    let id = store.subscribe(source)?;
    let mut feed = store
        .feeds()?
        .into_iter()
        .find(|feed| feed.id == id)
        .ok_or_else(|| anyhow::anyhow!("Demo subscription was not saved"))?;
    feed.title = title.to_owned();
    let articles = entries
        .iter()
        .enumerate()
        .map(|(index, (name, body))| IncomingArticle {
            body_origin: crate::model::FeedOrigin::Feed,
            external_id: format!("demo:{index}"),
            title: (*name).to_owned(),
            url: "https://example.org/lume-demo".into(),
            author: "Lume editorial · fictional sample".into(),
            published: 1_792_152_000 - (id * 3 + index as i64) * 3600,
            body: (*body).to_owned(),
        })
        .collect::<Vec<_>>();
    store.ingest(&feed, title, &articles)?;
    Ok(())
}

type Sample = (&'static str, &'static str);

const SOURCES: &[(&str, &str, &[Sample])] = &[
    (
        "https://example.org/systems.xml",
        "Systems & signals",
        &[
            (
                "The quiet elegance of doing less",
                include_str!("demo/systems.txt"),
            ),
            (
                "What a database promises when it says commit",
                include_str!("demo/transactions.txt"),
            ),
            (
                "Backpressure is a product decision",
                include_str!("demo/backpressure.txt"),
            ),
        ],
    ),
    (
        "https://example.org/rust.xml",
        "The Rust notebook",
        &[
            (
                "Ownership as a way to explain a program",
                include_str!("demo/ownership.txt"),
            ),
            (
                "A smaller API can carry a stronger promise",
                include_str!("demo/contracts.txt"),
            ),
        ],
    ),
    (
        "https://example.org/craft.xml",
        "Computing, slowly",
        &[(
            "Notes are part of the reading",
            include_str!("demo/notes.txt"),
        )],
    ),
];
