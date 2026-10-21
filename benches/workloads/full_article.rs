//! Full-page extraction, durable cache/search and repeated source switching.

use std::{hint::black_box, time::Instant};

use anyhow::{Context, Result, ensure};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lume::{
    app::{App, Focus},
    model::{
        ArticleQuery, ArticleTarget, ContentSource, ExtractedDocument, View,
    },
    source,
    store::Store,
    ui,
};
use ratatui::{Terminal, backend::TestBackend};

const DISCUSSION: &str = "[Comments](https://community.example/discussion)";

pub(super) fn run(iterations: usize, stress: bool) -> Result<()> {
    let html = fixture(stress)?;
    let document = extraction(&html, iterations)?;
    semantic_fallback(iterations, stress)?;
    let (directory, store, target) = library()?;
    store.cache_document(&target, &document)?;
    cache_search(&store, &target, &document, iterations)?;
    failure_preservation(&store, &target, &document, iterations)?;
    store.cache_document(&target, &document)?;
    source_switches(store, iterations)?;
    drop(directory);
    Ok(())
}

fn extraction(html: &str, iterations: usize) -> Result<ExtractedDocument> {
    let url = "https://journal.example/posts/queues/";
    let cold = Instant::now();
    let document = source::article::extract(html, url)?;
    println!(
        "full extraction input · html_bytes={} · markdown_bytes={} · first_ms={:.3}",
        html.len(),
        document.body.len(),
        cold.elapsed().as_secs_f64() * 1000.0
    );
    super::measure(
        "full-page extraction + safe normalization",
        iterations,
        || {
            let extracted = source::article::extract(black_box(html), url)?;
            ensure!(
                extracted.body.contains("~~~rust\n")
                    && extracted.body.contains("~~~sql\n"),
                "Code languages lost"
            );
            ensure!(
                extracted.body.contains("Bounded admission"),
                "Table content lost"
            );
            ensure!(
                !extracted.body.contains("Unrelated navigation marker")
                    && !extracted.body.contains("invisibleTrackingMarker"),
                "Page chrome leaked"
            );
            ensure!(
                extracted.body.len() <= 262_208,
                "Normalized body limit changed"
            );
            Ok(())
        },
    )?;
    Ok(document)
}

fn semantic_fallback(iterations: usize, stress: bool) -> Result<()> {
    let links = if stress { 5000 } else { 500 };
    let navigation = "<a href='/nav'>Navigation</a>".repeat(links);
    let html = include_str!("../../tests/fixtures/extraction/quote-main.html")
        .replace("<body>", &format!("<body><nav>{navigation}</nav>"));
    println!(
        "fallback input · navigation_links={links} · bytes={}",
        html.len()
    );
    super::measure(
        "semantic fallback after rejected footer",
        iterations,
        || {
            let document = source::article::extract(
                &html,
                "https://journal.example/report",
            )?;
            ensure!(
                document.body.contains("Request cleanup must inspect"),
                "Article lost"
            );
            ensure!(!document.body.contains("Copyright ©"), "Footer accepted");
            ensure!(!document.body.contains("Navigation"), "Navigation leaked");
            Ok(())
        },
    )
}

fn fixture(stress: bool) -> Result<String> {
    let template = include_str!("../../tests/fixtures/article.html");
    if !stress {
        return Ok(template.into());
    }
    // Repeat a fixed owned article body inside its original container. This
    // assembles representative long input, not an alternative HTML parser.
    let start = template.find("<p>").context("Fixture has no body")?;
    let end = template
        .find("</article>")
        .context("Fixture has no article end")?;
    let section = template.get(start..end).context("Invalid fixture range")?;
    // Keep a large successful page below the 256 KiB fragment limit, with
    // room for the extractor's generated wrappers. Oversize rejection is
    // qualified separately by the extraction and HTTP failure contracts.
    let copies = (224_usize * 1024)
        .saturating_sub(template.len())
        .checked_div(section.len())
        .context("Empty article fixture")?;
    ensure!(copies > 0, "Article fixture exceeds the stress budget");
    Ok(format!(
        "{}{}{}",
        &template[..start],
        section.repeat(copies),
        &template[end..]
    ))
}

fn library() -> Result<(tempfile::TempDir, Store, ArticleTarget)> {
    let directory = tempfile::tempdir()?;
    let mut store =
        Store::open(&directory.path().join("full-article.sqlite3"))?;
    store.subscribe("https://journal.example/feed")?;
    let feed = store.feeds()?.remove(0);
    let mut articles = source::parse(
        include_bytes!("../../tests/fixtures/rss.xml"),
        &feed.url,
    )?
    .articles;
    articles[0].body = DISCUSSION.into();
    store.ingest(&feed, "Systems journal", &articles)?;
    let article = store.library()?.articles.remove(0);
    let target = ArticleTarget {
        article: article.id,
        url: article.url,
    };
    Ok((directory, store, target))
}

fn cache_search(
    store: &Store,
    target: &ArticleTarget,
    document: &ExtractedDocument,
    iterations: usize,
) -> Result<()> {
    let query = ArticleQuery {
        view: View::All,
        text: "admission".into(),
        offset: 0,
        limit: 10,
        retained: None,
    };
    super::measure("full cache + FTS + revision stability", iterations, || {
        ensure!(
            store.cache_document(target, document)?,
            "Valid target discarded"
        );
        let page = store.query_articles(&query)?;
        ensure!(
            page.total == 1 && page.articles[0].id == target.article,
            "Full body was not indexed"
        );
        ensure!(
            store.body(target.article)? == DISCUSSION,
            "Feed text overwritten"
        );
        ensure!(
            store
                .cached_document(target.article)?
                .context("Cache lost")?
                .revision
                == 1,
            "Identical extraction changed revision"
        );
        Ok(())
    })
}

fn failure_preservation(
    store: &Store,
    target: &ArticleTarget,
    document: &ExtractedDocument,
    iterations: usize,
) -> Result<()> {
    let footer = ExtractedDocument {
        final_url: document.final_url.clone(),
        body: "Copyright © 2026. All rights reserved.".into(),
    };
    super::measure("failed fetch preserves cached article", iterations, || {
        ensure!(
            store.cache_document(target, &footer).is_err(),
            "Footer replaced article"
        );
        store.document_failure(target, "Controlled publisher failure")?;
        let retained = store
            .cached_document(target.article)?
            .context("Cache lost")?;
        ensure!(
            retained.body == document.body && retained.revision == 1,
            "Failure replaced cached text or revision"
        );
        ensure!(
            retained.error == "Controlled publisher failure",
            "Failure reason lost"
        );
        Ok(())
    })
}

fn source_switches(store: Store, iterations: usize) -> Result<()> {
    let mut app = App::new(store, false)?;
    app.focus = Focus::Reader;
    app.resize_reader(78, 29);
    let mut terminal = Terminal::new(TestBackend::new(144, 44))?;
    super::measure("Feed/Full switches + cached frame", iterations, || {
        command(&mut app, ":content feed")?;
        ensure!(
            app.body == DISCUSSION && app.content_source == ContentSource::Feed,
            "Feed source unavailable"
        );
        command(&mut app, ":content full")?;
        ensure!(
            app.content_source == ContentSource::Full
                && app.body.contains("Preserve the contract"),
            "Full source unavailable"
        );
        ensure!(
            app.links
                .iter()
                .any(|(_, url)| url == "https://community.example/discussion"),
            "Community discussion link lost"
        );
        terminal.draw(|frame| ui::draw(frame, &app))?;
        let screen = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        ensure!(
            screen.contains("Full article cached"),
            "Full source badge missing"
        );
        Ok(())
    })
}

fn command(app: &mut App, text: &str) -> Result<()> {
    super::type_keys(app, text)?;
    app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?;
    Ok(())
}
