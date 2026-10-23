//! A fresh ordinary cache must not parse every historical article image link.

use anyhow::{Result, ensure};
use lume::{
    config::{self, Session, retention::Age},
    model::{FeedOrigin, IncomingArticle, View},
    retention,
    store::Store,
};
use std::path::Path;

pub fn run(iterations: usize, stress: bool) -> Result<()> {
    let directory = tempfile::tempdir()?;
    let count = if stress { 2000 } else { 200 };
    let store = prepare(directory.path(), count)?;
    let images = directory.path().join("images");
    let now = chrono::Utc::now().timestamp();
    super::measure(
        &format!("fresh image retention preview ({count} articles, 32 files)"),
        iterations,
        || {
            let report = retention::preview(&store, &images, now)?;
            ensure!(
                report.images == 0 && report.articles == 0,
                "Fresh content expired"
            );
            ensure!(store.view_count(&View::All)? == count, "Library changed");
            ensure!(
                lume::media::cache::path(&images, "https://images.example/1/0")
                    .is_file(),
                "Fresh cache lost"
            );
            Ok(())
        },
    )?;
    age_images(&images)?;
    ensure!(
        retention::preview(&store, &images, now)?.images == 32,
        "Index lost expired references"
    );
    super::measure(
        &format!("indexed old image retention ({count} articles, 32 files)"),
        iterations,
        || {
            ensure!(
                retention::preview(&store, &images, now)?.images == 32,
                "Index changed expiry"
            );
            Ok(())
        },
    )
}

fn prepare(path: &Path, count: usize) -> Result<Store> {
    let database = path.join("library.sqlite3");
    let mut store = Store::open(&database)?;
    store.subscribe("https://publisher.example/feed")?;
    let feed = store.feeds()?.remove(0);
    let entries = (1..=count).map(article).collect::<Vec<_>>();
    store.ingest(&feed, "Gallery feed", &entries)?;
    rusqlite::Connection::open(&database)?.execute_batch(include_str!(
        "../../src/store/sql/bench_age_images.sql"
    ))?;
    let document = path.join("config.toml");
    let session = Session::open(&store, &document)?;
    let mut config = config::decode(&session.initial().document)?;
    config.retention.defaults.image_days = Age(Some(30));
    std::fs::write(&document, config::encode(&config)?)?;
    Session::open(&store, &document)?;
    for index in 1..=32 {
        lume::media::cache::write(
            &path.join("images"),
            &format!("https://images.example/{index}/0"),
            b"Cached image fixture",
        )?;
    }
    Ok(store)
}

fn article(id: usize) -> IncomingArticle {
    IncomingArticle {
        external_id: format!("gallery:{id}"),
        title: format!("Gallery {id}"),
        url: format!("https://publisher.example/{id}"),
        author: "Publisher".into(),
        published: 1_000_000_000,
        body: (0..20)
            .map(|image| {
                format!("![Image](https://images.example/{id}/{image})\n\n")
            })
            .collect(),
        body_origin: FeedOrigin::Feed,
    }
}

fn age_images(images: &Path) -> Result<()> {
    for entry in std::fs::read_dir(images)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            std::fs::File::open(entry.path())?.set_modified(
                std::time::UNIX_EPOCH
                    + std::time::Duration::from_secs(1_000_000_000),
            )?;
        }
    }
    Ok(())
}
