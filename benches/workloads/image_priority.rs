//! Visible pixels after obsolete sources use real cache, layout and worker lanes.
use anyhow::{Context, Result, ensure};
use lume::{
    app::{App, Focus},
    media,
    model::IncomingArticle,
    store::Store,
};
use ratatui::style::Color;
use std::time::{Duration, Instant};

pub(super) fn run(iterations: usize, stress: bool) -> Result<()> {
    let count = iterations.min(8);
    let directory = tempfile::tempdir()?;
    let cache = directory.path().join("images");
    let mut store = Store::open(&directory.path().join("library.sqlite3"))?;
    let targets = seed(&mut store, &cache, count, stress)?;
    let mut app = App::new(store, false)?;
    app.focus = Focus::Reader;
    app.resize_reader(80, 40);
    app.enable_images(cache, true)?;
    let mut elapsed = Duration::ZERO;
    for (old, target) in targets {
        app.jump_article(old)?;
        app.poll_images();
        app.poll_images();
        let start = Instant::now();
        app.jump_article(target)?;
        app.focus = Focus::Reader;
        settle(&mut app)?;
        elapsed += start.elapsed();
        let key = app.image_key(&app.images.slots[0]).context("Visible key")?;
        let image = app
            .images
            .thumbnail(&key)
            .context("Visible pixels missing")?;
        ensure!(
            image
                .cells
                .content
                .iter()
                .any(|cell| cell.bg == Color::Rgb(0, 0, 255)),
            "Visible source color changed"
        );
    }
    println!(
        "visible image after replacing two sources: {count} iterations, {:.3} ms total, {:.3} ms/iteration",
        elapsed.as_secs_f64() * 1000.0,
        elapsed.as_secs_f64() * 1000.0 / count as f64
    );
    Ok(())
}

fn settle(app: &mut App) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        app.poll_images();
        let key = app
            .image_key(&app.images.slots[0])
            .context("Target image")?;
        if app.images.thumbnail(&key).is_some() {
            return Ok(());
        }
        ensure!(Instant::now() < deadline, "Visible image starved");
        std::thread::sleep(Duration::from_micros(100));
    }
}

fn seed(
    store: &mut Store,
    cache: &std::path::Path,
    count: usize,
    stress: bool,
) -> Result<Vec<(i64, i64)>> {
    store.subscribe("https://journal.test/feed")?;
    let feed = store.feeds()?.remove(0);
    let mut targets = Vec::new();
    for index in 0..count {
        let urls = (0..3)
            .map(|slot| format!("https://image.test/{index}/{slot}"))
            .collect::<Vec<_>>();
        cache_sources(cache, &urls, stress)?;
        let old = article(
            index * 2,
            format!("![Old A]({})\n\n![Old B]({})", urls[0], urls[1]),
        );
        let target = article(index * 2 + 1, format!("![Visible]({})", urls[2]));
        store.ingest(&feed, &feed.title, &[old, target])?;
        let articles = store.library()?.articles;
        let old = articles
            .iter()
            .find(|article| article.title == format!("Article {}", index * 2))
            .context("Old article")?
            .id;
        let target = articles
            .iter()
            .find(|article| {
                article.title == format!("Article {}", index * 2 + 1)
            })
            .context("New article")?
            .id;
        targets.push((old, target));
    }
    Ok(targets)
}

fn article(index: usize, body: String) -> IncomingArticle {
    IncomingArticle {
        external_id: format!("article:{index}"),
        title: format!("Article {index}"),
        url: format!("https://journal.test/{index}"),
        author: String::new(),
        published: 1_800_000_000 + index as i64,
        body,
        body_origin: lume::model::FeedOrigin::Feed,
    }
}

fn svg(count: usize, slot: usize) -> String {
    let color = ["red", "green", "blue"][slot];
    let mut source = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="600"><rect width="1200" height="600" fill="{color}"/>"#
    );
    if slot != 2 {
        for index in 0..count {
            source.push_str(&format!(
                r#"<path d="M{} {}q40 30 10 70t60 20z" fill="{color}"/>"#,
                index % 80 * 15,
                index / 80 * 15
            ));
        }
    }
    source.push_str("</svg>");
    source
}

fn cache_sources(
    cache: &std::path::Path,
    urls: &[String],
    stress: bool,
) -> Result<()> {
    for (slot, url) in urls.iter().enumerate() {
        let bytes = svg(
            if slot == 2 {
                1
            } else if stress {
                3000
            } else {
                800
            },
            slot,
        );
        media::cache::write(cache, url, bytes.as_bytes())?;
    }
    Ok(())
}
