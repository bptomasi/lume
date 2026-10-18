//! Correctness-checked search, organization, study and interaction workloads.
//!
//! Interaction times cover application input through a TestBackend frame. They
//! exclude physical keyboard delivery, terminal rasterization and network I/O.

use std::{collections::BTreeSet, time::Instant};

use anyhow::{Context, Result, ensure};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lume::{
    app::{App, Focus},
    exchange,
    model::{ArticleQuery, BulkEdit, FeedOrigin, IncomingArticle, View},
    source,
    store::Store,
    ui,
};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};

#[path = "expansion_background.rs"]
mod background;

pub(super) fn run(iterations: usize, stress: bool) -> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("expansion.sqlite3");
    let mut store = Store::open(&path)?;
    let count = if stress { 20_000 } else { 2_000 };
    super::seed(&mut store, count)?;
    seed_reader(&mut store, stress)?;
    println!(
        "expansion input · metadata_articles={} · stress={stress}",
        count + 1
    );
    advanced_search(&store, count, iterations)?;
    anchored_page(&store, count, iterations)?;
    extraction_rule(iterations, stress)?;
    bulk_undo(&store, iterations, stress)?;
    study_cards(&store, iterations)?;
    incremental_export(&store, directory.path(), iterations)?;
    interactions(store, &path, iterations * 5, stress)?;
    Ok(())
}

fn anchored_page(store: &Store, count: usize, iterations: usize) -> Result<()> {
    let total = count.div_ceil(2);
    let query = ArticleQuery {
        view: View::All,
        text: "title:systems".into(),
        offset: total - 1,
        limit: 100,
        retained: None,
    };
    let target = store.query_articles(&query)?.articles.remove(0).id;
    let expected_offset = (total - 1) / 100 * 100;
    super::measure("anchored filtered page + stable target", iterations, || {
        let (offset, page) = store
            .page_containing(&query, target)?
            .context("Matching anchor disappeared")?;
        ensure!(offset == expected_offset, "Wrong anchored page offset");
        ensure!(page.total == total, "Anchored count escaped the filter");
        ensure!(
            page.articles.iter().any(|article| article.id == target),
            "Anchored page lost its captured target"
        );
        Ok(())
    })
}

fn seed_reader(store: &mut Store, stress: bool) -> Result<()> {
    let feed = store.feeds()?.remove(0);
    store.ingest(
        &feed,
        &feed.title,
        &[IncomingArticle {
            external_id: "reader-document".into(),
            title: "Measured reader document".into(),
            url: "https://benchmark.example/reader".into(),
            author: "Lume original fixture".into(),
            published: i64::MAX / 2,
            body: include_str!("../fixtures/rich_article.md")
                .repeat(if stress { 200 } else { 20 }),
            body_origin: FeedOrigin::Feed,
        }],
    )
}

fn advanced_search(
    store: &Store,
    count: usize,
    iterations: usize,
) -> Result<()> {
    super::measure(
        "boolean field search + complete count/page",
        iterations,
        || {
            let page = store.query_articles(&ArticleQuery {
                view: View::All,
                text: "(title:systems OR title:missing) AND NOT title:rust"
                    .into(),
                offset: 0,
                limit: 100,
                retained: None,
            })?;
            ensure!(page.total == count.div_ceil(2), "Wrong boolean count");
            ensure!(
                page.articles.iter().all(|article| article
                    .title
                    .starts_with("systems article")),
                "Boolean query leaked another topic"
            );
            Ok(())
        },
    )
}

fn extraction_rule(iterations: usize, stress: bool) -> Result<()> {
    let paragraph = "<p>A transaction must own its durable write boundary and its failure recovery path.</p>";
    let html = include_str!("../../tests/fixtures/article.html").replace(
        "</article>",
        &format!(
            "{}</article>",
            paragraph.repeat(if stress { 1000 } else { 10 })
        ),
    );
    super::measure("reviewable CSS article extraction", iterations, || {
        let document = source::article::extract_with_selector(
            &html,
            "https://journal.example/a",
            "article",
        )?;
        ensure!(
            document.body.contains("~~~rust")
                && document.body.contains("~~~sql"),
            "Rule lost language metadata"
        );
        ensure!(
            document.body.contains("durable write boundary"),
            "Rule lost main text"
        );
        ensure!(
            !document.body.contains("Unrelated navigation marker"),
            "Rule leaked navigation"
        );
        Ok(())
    })
}

fn bulk_undo(store: &Store, iterations: usize, stress: bool) -> Result<()> {
    let count = if stress { 500 } else { 100 };
    let selected = (1..=count).collect::<BTreeSet<i64>>();
    super::measure("captured favorite batch + atomic undo", iterations, || {
        store.edit_articles(&selected, &BulkEdit::Favorite(true))?;
        ensure!(
            store.view_count(&View::Starred)? == count as usize,
            "Batch changed wrong identities"
        );
        ensure!(store.undo()?, "Missing batch inverse");
        ensure!(
            store.view_count(&View::Starred)? == 0,
            "Batch inverse lost flags"
        );
        Ok(())
    })
}

fn study_cards(store: &Store, iterations: usize) -> Result<()> {
    store.save_comment(
        1,
        None,
        "Explain ownership at the durable write boundary.",
    )?;
    let comment = store.comments(1)?[0].id;
    super::measure(
        "user card creation/review + complete undo",
        iterations,
        || {
            store.create_study_card(
                1,
                comment,
                "Who owns a durable write?",
                "The transaction owner.",
            )?;
            let card = store.annotation(1, comment)?.cards.remove(0);
            store.review_study_card(&card, 3, 100)?;
            ensure!(
                store.annotation(1, comment)?.cards[0].due_at == 259_300,
                "Wrong review deadline"
            );
            ensure!(store.undo()? && store.undo()?, "Study history incomplete");
            ensure!(
                store.annotation(1, comment)?.cards.is_empty(),
                "Card leaked after undo"
            );
            Ok(())
        },
    )?;
    Ok(())
}

fn incremental_export(
    store: &Store,
    directory: &std::path::Path,
    iterations: usize,
) -> Result<()> {
    let output = directory.join("vault");
    ensure!(
        exchange::incremental::notebook(store, &output)?.written == 1,
        "Initial export missing"
    );
    super::measure(
        "unchanged incremental notebook reconciliation",
        iterations,
        || {
            let report = exchange::incremental::notebook(store, &output)?;
            ensure!(
                report.unchanged == 1
                    && report.written == 0
                    && report.conflicts.is_empty(),
                "Unchanged export rewrote a file"
            );
            Ok(())
        },
    )
}

fn interactions(
    store: Store,
    path: &std::path::Path,
    samples: usize,
    stress: bool,
) -> Result<()> {
    let mut app = App::new(store, false)?;
    app.set_view(View::All)?;
    app.focus = Focus::Reader;
    app.resize_reader(82, 29);
    let mut terminal = Terminal::new(TestBackend::new(144, 44))?;
    latency("reader motion to frame", samples, |index| {
        let before = app.reader_cursor;
        press(
            &mut app,
            if index % 2 == 0 {
                KeyCode::Down
            } else {
                KeyCode::Up
            },
        )?;
        terminal.draw(|frame| ui::draw(frame, &app))?;
        ensure!(app.reader_cursor != before, "Reader motion was lost");
        Ok(())
    })?;
    app.focus = Focus::Articles;
    latency("cached article selection/open to frame", samples, |_| {
        let before = app.current().context("Missing selection")?.id;
        press(&mut app, KeyCode::Down)?;
        press(&mut app, KeyCode::Enter)?;
        terminal.draw(|frame| ui::draw(frame, &app))?;
        ensure!(
            app.current().context("Selection lost")?.id < before,
            "Open retargeted the article"
        );
        app.focus = Focus::Articles;
        Ok(())
    })?;
    press(&mut app, KeyCode::Home)?;
    app.focus = Focus::Reader;
    reflow_latency(&mut app, &mut terminal, samples)?;
    background::run(&mut app, &mut terminal, path, samples, stress)
}

fn reflow_latency(
    app: &mut App,
    terminal: &mut Terminal<TestBackend>,
    samples: usize,
) -> Result<()> {
    latency(
        "responsive resize/reflow to themed frame",
        samples,
        |index| {
            let (width, height) =
                if index % 2 == 0 { (80, 24) } else { (144, 44) };
            app.workspace.theme =
                if index % 2 == 0 { "nord" } else { "lume" }.into();
            terminal.backend_mut().resize(width, height);
            terminal.autoresize()?;
            let area = ui::workspace_areas(
                Rect::new(0, 0, width, height),
                app.focus,
                &app.workspace,
            )
            .reader
            .context("Reader disappeared")?;
            let content = ui::workspace_reader_content(area, &app.workspace);
            app.resize_reader(content.width.into(), content.height.into());
            terminal.draw(|frame| ui::draw(frame, app))?;
            ensure!(
                app.reader_text.contains("Systems"),
                "Resize discarded article text"
            );
            Ok(())
        },
    )
}

fn press(app: &mut App, code: KeyCode) -> Result<()> {
    app.key(KeyEvent::new(code, KeyModifiers::NONE))?;
    Ok(())
}

fn latency(
    name: &str,
    samples: usize,
    mut action: impl FnMut(usize) -> Result<()>,
) -> Result<()> {
    let mut elapsed = Vec::with_capacity(samples.max(2));
    for index in 0..samples.max(2) {
        let start = Instant::now();
        action(index)?;
        elapsed.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    elapsed.sort_by(f64::total_cmp);
    let percentile = |percent: usize| {
        elapsed[(elapsed.len() * percent).div_ceil(100).saturating_sub(1)]
    };
    println!(
        "{name}: samples={} p50_ms={:.3} p95_ms={:.3} p99_ms={:.3}",
        elapsed.len(),
        percentile(50),
        percentile(95),
        percentile(99)
    );
    Ok(())
}
