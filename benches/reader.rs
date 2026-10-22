//! Correctness-checked workloads for the reader's hot paths.

use std::{hint::black_box, time::Instant};

use anyhow::{Result, ensure};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lume::{app::App, model::IncomingArticle, source, store::Store, ui};
use ratatui::{Terminal, backend::TestBackend};

#[path = "workloads/notebook.rs"]
mod annotation_notebook;
#[path = "workloads/annotation_projection.rs"]
mod annotation_projection;
#[path = "workloads/catalog.rs"]
mod catalog;
#[path = "workloads/code_viewport.rs"]
mod code_viewport;
#[path = "workloads/configuration.rs"]
mod configuration;
#[path = "workloads/dense_annotations.rs"]
mod dense_annotations;
#[path = "workloads/expansion.rs"]
mod expansion;
#[path = "workloads/feed_content.rs"]
mod feed_content;
#[path = "workloads/fluency.rs"]
mod fluency;
#[path = "workloads/formulas.rs"]
mod formulas;
#[path = "workloads/full_article.rs"]
mod full_article;
#[path = "workloads/geometry.rs"]
mod geometry;
#[path = "workloads/highlighting.rs"]
mod highlighting;
#[path = "workloads/images.rs"]
mod images;
#[path = "workloads/interaction.rs"]
mod interaction;
#[path = "workloads/links.rs"]
mod links;
#[path = "workloads/lists.rs"]
mod lists;
#[path = "workloads/note_interactions.rs"]
mod note_interactions;
#[path = "workloads/note_study.rs"]
mod note_study;
#[path = "workloads/pane_search.rs"]
mod pane_search;
#[path = "workloads/panes.rs"]
mod panes;
#[path = "workloads/presentation.rs"]
mod presentation;
#[path = "workloads/query_editing.rs"]
mod query_editing;
#[path = "workloads/reading_services.rs"]
mod reading_services;
#[path = "workloads/reading_tools.rs"]
mod reading_tools;
#[path = "workloads/recommendations.rs"]
mod recommendations;
#[path = "workloads/recovery_navigation.rs"]
mod recovery_navigation;
#[path = "workloads/rich_text.rs"]
mod rich_text;
#[path = "workloads/search_relevance.rs"]
mod search_relevance;
#[path = "workloads/service_interaction.rs"]
mod service_interaction;
#[path = "workloads/smoothness.rs"]
mod smoothness;
#[path = "workloads/tables.rs"]
mod tables;
#[path = "workloads/welcome.rs"]
mod welcome;
#[path = "workloads/workspace_reflow.rs"]
mod workspace_reflow;
#[path = "workloads/workspaces.rs"]
mod workspaces;

fn main() -> Result<()> {
    let verify = std::env::args().any(|argument| argument == "--test");
    let stress = std::env::args().any(|argument| argument == "--stress");
    let count = if verify {
        100
    } else if stress {
        50_000
    } else {
        10_000
    };
    let iterations = if verify { 2 } else { 40 };
    if let Some(result) = focused_workload(iterations, stress) {
        return result;
    }
    let directory = tempfile::tempdir()?;
    let mut store = Store::open(&directory.path().join("benchmark.sqlite3"))?;
    seed(&mut store, count)?;
    if std::env::args().any(|argument| argument == "--navigation-only") {
        return navigation_workload(store, count, iterations);
    }
    component_workloads(iterations, stress)?;
    println!("lume baseline · articles={count} · iterations={iterations}");
    metadata(&store, count, iterations)?;
    annotations(&store, iterations)?;
    parsing(if stress { 2000 } else { 200 }, iterations)?;
    feed_content::run(iterations, stress)?;
    full_article::run(iterations, stress)?;
    expansion::run(iterations, stress)?;
    indexed_search(&store, count, iterations)?;
    read_destination(&store, count, iterations)?;
    rich_content(iterations)?;
    highlighting::run(iterations, stress)?;
    portable_library(iterations, stress)?;
    notebook(&store, iterations, stress)?;
    presentation(store, count, iterations)?;
    Ok(())
}

fn component_workloads(iterations: usize, stress: bool) -> Result<()> {
    geometry::run(iterations, stress)?;
    reading_services::run(iterations, stress)?;
    service_interaction::run(iterations, stress)?;
    recovery_navigation::run(iterations, stress)?;
    annotation_projection::run(iterations, stress)?;
    dense_annotations::run(iterations, stress)?;
    rich_text::run(iterations, stress)?;
    lists::run(iterations, stress)?;
    configuration::run(iterations, stress)?;
    tables::run(iterations, stress)?;
    links::run(iterations, stress)?;
    presentation::run(iterations, stress)?;
    welcome::run(iterations, stress)?;
    code_viewport::run(iterations, stress)?;
    interaction::run(iterations, stress)?;
    reading_selection(iterations)?;
    catalog::run(iterations, stress)?;
    reading_tools::run(iterations, stress)?;
    smoothness::run(iterations, stress)?;
    panes::run(iterations, stress)?;
    pane_search::run(iterations, stress)?;
    fluency::run(iterations, stress)?;
    formulas::run(iterations, stress)?;
    query_editing::run(iterations, stress)?;
    search_relevance::run(iterations, stress)?;
    workspaces::run(iterations, stress)?;
    Ok(())
}

type Workload = fn(usize, bool) -> Result<()>;

const FOCUSED_WORKLOADS: &[(&str, Workload)] = &[
    ("--geometry-only", geometry::run),
    ("--reading-services-only", reading_services::run),
    ("--service-interaction-only", service_interaction::run),
    ("--annotation-projection-only", annotation_projection::run),
    ("--recovery-navigation-only", recovery_navigation::run),
    ("--dense-annotations-only", dense_annotations::run),
    ("--rich-text-only", rich_text::run),
    ("--lists-only", lists::run),
    ("--configuration-only", configuration::run),
    ("--welcome-only", welcome::run),
    ("--interaction-only", interaction::run),
    ("--code-viewport-only", code_viewport::run),
    ("--images-only", images::run),
    ("--recommendations-only", recommendations::run),
    ("--notebook-only", annotation_notebook::run),
    ("--notes-only", note_interactions::run),
    ("--workspaces-only", workspaces::run),
    ("--panes-only", panes::run),
    ("--pane-search-only", pane_search::run),
    ("--fluency-only", fluency::run),
    ("--formulas-only", formulas::run),
    ("--query-editing-only", query_editing::run),
    ("--search-relevance-only", search_relevance::run),
    ("--smoothness-only", smoothness::run),
    ("--expansion-only", expansion::run),
    ("--full-article-only", full_article::run),
    ("--feed-content-only", feed_content::run),
    ("--archive-feeds-only", feed_content::archives),
    ("--tables-only", tables::run),
    ("--links-only", links::run),
    ("--presentation-only", presentation::run),
    ("--highlighting-only", highlighting::run),
    ("--reading-tools-only", reading_tools::run),
    ("--catalog-only", catalog::run),
];

fn focused_workload(iterations: usize, stress: bool) -> Option<Result<()>> {
    let arguments = std::env::args().collect::<Vec<_>>();
    FOCUSED_WORKLOADS
        .iter()
        .find(|(flag, _)| arguments.iter().any(|argument| argument == flag))
        .map(|(_, workload)| workload(iterations, stress))
}

fn seed(store: &mut Store, count: usize) -> Result<()> {
    store.subscribe("https://benchmark.example/feed")?;
    let feed = store.feeds()?.remove(0);
    for start in (0..count).step_by(2000) {
        let articles = (start..(start + 2000).min(count))
            .map(|index| {
                let topic = if index % 2 == 0 { "systems" } else { "rust" };
                IncomingArticle {
                    body_origin: lume::model::FeedOrigin::Feed,
                    external_id: index.to_string(),
                    title: format!("{topic} article {index}"),
                    url: format!("https://benchmark.example/article/{index}"),
                    author: "Benchmark author".into(),
                    published: 1_792_152_000 + index as i64,
                    body: include_str!("../src/demo/systems.txt").into(),
                }
            })
            .collect::<Vec<_>>();
        store.ingest(&feed, "Benchmark journal", &articles)?;
    }
    Ok(())
}

fn metadata(store: &Store, count: usize, iterations: usize) -> Result<()> {
    measure("metadata snapshot (no bodies)", iterations, || {
        let library = black_box(store.library()?);
        ensure!(
            library.articles.len() == count.min(10_000),
            "Wrong snapshot size"
        );
        ensure!(
            library.articles[0]
                .title
                .ends_with(&(count - 1).to_string()),
            "Wrong sort order"
        );
        Ok(())
    })
}

fn annotations(store: &Store, iterations: usize) -> Result<()> {
    let article = store.library()?.articles[0].id;
    let list = store.create_collection("Measured list")?;
    store.save_comment(article, None, "Before")?;
    let comment = store.comments(article)?[0].id;
    measure("durable comment/favorite/list cycle", iterations, || {
        store.save_comment(article, Some(comment), "After")?;
        store.set_starred(article, true)?;
        store.add_member(list, article)?;
        ensure!(store.comments(article)?[0].body == "After", "Comment lost");
        store.remove_member(list, article)?;
        store.set_starred(article, false)?;
        store.set_queued(article, true)?;
        ensure!(store.undo()?, "Queue update had no inverse");
        ensure!(
            !store.article(article)?.queued,
            "Undo retained queue membership"
        );
        Ok(())
    })
}

fn parsing(entries: usize, iterations: usize) -> Result<()> {
    // Compact publisher-style XML is an exact workload fixture. Keep its bytes
    // stable when comparing recorded baselines, including whitespace.
    let mut rss = String::from(
        "<rss version=\"2.0\"><channel><title>Measured feed</title><link>https://benchmark.example</link><description>Computing</description>",
    );
    for index in 0..entries {
        rss.push_str(&format!(
            "<item><guid>{index}</guid><title>Entry {index}</title><description><![CDATA[<h2>Reasoning about work</h2><p>A useful article &amp; a question.</p><pre>let answer = 42;</pre>]]></description></item>"
        ));
    }
    rss.push_str("</channel></rss>");
    println!("feed parse input · entries={entries} · bytes={}", rss.len());
    measure("RSS decode + content normalization", iterations, || {
        let parsed = black_box(source::parse(
            rss.as_bytes(),
            "https://benchmark.example/feed",
        )?);
        ensure!(parsed.articles.len() == entries, "Wrong entry count");
        ensure!(
            parsed.articles[0]
                .body
                .contains("A useful article & a question."),
            "Content lost"
        );
        Ok(())
    })
}

fn presentation(store: Store, count: usize, iterations: usize) -> Result<()> {
    let mut app = App::new(store, false)?;
    app.resize_reader(78, 29);
    let mut terminal = Terminal::new(TestBackend::new(144, 44))?;
    measure("three-pane frame (144x44)", iterations * 5, || {
        terminal.draw(|frame| ui::draw(frame, &app))?;
        ensure!(
            terminal.backend().buffer()[(0, 2)].symbol() == "╭",
            "Pane border missing"
        );
        Ok(())
    })?;
    measure("FTS live search + selected body", iterations, || {
        type_keys(&mut app, "/systems")?;
        ensure!(
            app.page_total == count / 2,
            "Search returned wrong articles"
        );
        app.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))?;
        Ok(())
    })?;
    command_menu(&mut app, iterations)?;
    released_input(&mut app, iterations)?;
    fullscreen(&mut app, iterations)
}

fn command_menu(app: &mut App, iterations: usize) -> Result<()> {
    let mut terminal = Terminal::new(TestBackend::new(120, 32))?;
    measure(
        "command filtering + menu frame (120x32)",
        iterations,
        || {
            type_keys(app, ":expor")?;
            let choices = lume::app::command_suggestions("expor");
            ensure!(
                choices.iter().map(|choice| choice.name).collect::<Vec<_>>()
                    == vec![
                        "export",
                        "export-notebook",
                        "export-opml",
                        "export-notes",
                        "export-note-files"
                    ],
                "Export command choices changed"
            );
            terminal.draw(|frame| ui::draw(frame, app))?;
            app.key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE))?;
            ensure!(
                matches!(&app.mode, lume::app::Mode::Input(input)
                if input.text == "export "),
                "Completion lost its argument separator"
            );
            app.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))?;
            Ok(())
        },
    )
}

fn type_keys(app: &mut App, keys: &str) -> Result<()> {
    for character in keys.chars() {
        app.key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE))?;
    }
    Ok(())
}

fn measure(
    label: &str,
    iterations: usize,
    mut operation: impl FnMut() -> Result<()>,
) -> Result<()> {
    let started = Instant::now();
    for _ in 0..iterations {
        operation()?;
    }
    let elapsed = started.elapsed().as_secs_f64();
    println!(
        "{label}: {iterations} iterations, {:.3} ms total, {:.3} ms/iteration",
        elapsed * 1000.0,
        elapsed * 1000.0 / iterations as f64
    );
    Ok(())
}

fn indexed_search(
    store: &Store,
    count: usize,
    iterations: usize,
) -> Result<()> {
    use lume::model::{ArticleQuery, View};
    measure("indexed body search + last page", iterations, || {
        let page = store.query_articles(&ArticleQuery {
            view: View::All,
            text: "systems".into(),
            offset: count / 2 - 10,
            limit: 10,
            retained: None,
        })?;
        ensure!(
            page.total == count / 2,
            "Incorrect full-library match count"
        );
        ensure!(
            page.articles.len() == 10,
            "Old matching articles inaccessible"
        );
        Ok(())
    })
}

fn rich_content(iterations: usize) -> Result<()> {
    let source = include_str!("fixtures/rich_article.md");
    let _ = lume::reader::render(source, 72);
    measure("Markdown wrap + syntax highlight", iterations, || {
        let rendered = lume::reader::render(black_box(source), 72);
        ensure!(rendered.links.len() == 1, "Reader lost its link");
        ensure!(rendered.code.iter().any(|code| *code), "Code block lost");
        Ok(())
    })
}

fn portable_library(iterations: usize, stress: bool) -> Result<()> {
    let count = if stress { 5000 } else { 1000 };
    let mut opml = String::from("<opml version=\"2.0\"><body>");
    for index in 0..count {
        opml.push_str(&format!(
            "<outline text=\"Source {index}\" xmlUrl=\"https://publisher.example/{index}/rss\"/>"
        ));
    }
    opml.push_str("</body></opml>");
    measure("OPML decode + URL validation", iterations, || {
        let feeds = lume::exchange::opml::decode(black_box(&opml))?;
        ensure!(feeds.len() == count, "OPML lost subscriptions");
        Ok(())
    })?;
    let mut html = String::from("<html><head>");
    for index in 0..100 {
        html.push_str(&format!(
            "<link rel=alternate type=application/rss+xml href=/feeds/{index}>"
        ));
    }
    html.push_str("</head></html>");
    measure("HTML5 discovery (100 advertised feeds)", iterations, || {
        let feeds = source::discovery::links(
            black_box(&html),
            "https://publisher.example/blog",
        )?;
        ensure!(feeds.len() == 100, "Discovery lost feeds");
        ensure!(feeds[99].url.ends_with("/feeds/99"), "Incorrect resolution");
        Ok(())
    })
}

fn notebook(store: &Store, iterations: usize, stress: bool) -> Result<()> {
    let count = if stress { 500 } else { 100 };
    seed_notebook(store, count)?;
    let directory = tempfile::tempdir()?;
    let mut run = 0;
    measure(
        "stream notebook export (2 notes/article)",
        iterations,
        || {
            let path = directory.path().join(format!("notebook-{run}.md"));
            run += 1;
            lume::exchange::markdown::notebook(store, &path)?;
            let text = std::fs::read_to_string(&path)?;
            ensure!(
                text.matches("> An immutable source passage").count() == count,
                "Notebook lost captured quotes"
            );
            std::fs::remove_file(path)?;
            Ok(())
        },
    )
}

fn seed_notebook(store: &Store, count: usize) -> Result<()> {
    use lume::model::{ArticleQuery, View};
    let articles = store
        .query_articles(&ArticleQuery {
            view: View::All,
            text: String::new(),
            offset: 0,
            limit: count,
            retained: None,
        })?
        .articles;
    for article in &articles {
        store.save_highlight(
            article.id,
            "An immutable source passage",
            0,
            "Compare this idea with the next article.",
        )?;
        store.save_comment(
            article.id,
            None,
            "Keep the supporting evidence.",
        )?;
    }
    Ok(())
}

fn reading_selection(iterations: usize) -> Result<()> {
    let directory = tempfile::tempdir()?;
    let store = Store::open(&directory.path().join("reading.sqlite3"))?;
    let initial = lume::catalog::install(&store)?;
    ensure!(
        initial.total_feeds == 98,
        "Reading selection changed unexpectedly"
    );
    ensure!(initial.added_searches == 46, "Topic searches missing");
    measure(
        "bundled catalog + idempotent installation",
        iterations,
        || {
            let repeated = lume::catalog::install(&store)?;
            ensure!(repeated.added_feeds == 0, "Subscriptions duplicated");
            ensure!(
                repeated.added_searches == 0,
                "Existing searches overwritten"
            );
            ensure!(store.feeds()?.len() == 98, "Source selection lost");
            Ok(())
        },
    )
}

fn read_destination(
    store: &Store,
    count: usize,
    iterations: usize,
) -> Result<()> {
    use lume::model::{ArticleQuery, View};
    let mut query = ArticleQuery {
        view: View::All,
        text: String::new(),
        offset: count.saturating_sub(500),
        limit: 500,
        retained: None,
    };
    let articles = store.query_articles(&query)?.articles;
    for article in &articles {
        store.set_unread(article.id, false)?;
    }
    query.view = View::Read;
    query.offset = articles.len().saturating_sub(10);
    query.limit = 10;
    let expected = articles
        .iter()
        .skip(query.offset)
        .map(|article| article.id)
        .collect::<Vec<_>>();
    let measured =
        measure("read folder count + oldest page", iterations, || {
            let page = store.query_articles(&query)?;
            ensure!(page.total == articles.len(), "Read count drifted");
            let actual = page
                .articles
                .iter()
                .map(|article| article.id)
                .collect::<Vec<_>>();
            ensure!(actual == expected, "Read folder lost old articles");
            Ok(())
        });
    for article in &articles {
        store.set_unread(article.id, true)?;
    }
    measured
}

fn fullscreen(app: &mut App, iterations: usize) -> Result<()> {
    let mut terminal = Terminal::new(TestBackend::new(160, 50))?;
    let article = app.current().map(|article| article.id);
    measure(
        "fullscreen reflow + frame + restore (160x50)",
        iterations,
        || {
            type_keys(app, "Z")?;
            let screen = ratatui::layout::Rect::new(0, 0, 160, 50);
            let content = ui::workspace_reader_content(screen, &app.workspace);
            app.resize_reader(content.width.into(), content.height.into());
            terminal.draw(|frame| ui::draw(frame, app))?;
            ensure!(app.workspace.fullscreen, "Fullscreen did not open");
            ensure!(
                content.width == 80 && content.x == 40,
                "Fullscreen prose must occupy the centered 80-column block"
            );
            ensure!(
                terminal.backend().buffer()[(0, 0)].symbol() == "╭",
                "Article did not occupy the terminal"
            );
            app.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))?;
            ensure!(!app.workspace.fullscreen, "Fullscreen did not close");
            ensure!(
                app.current().map(|article| article.id) == article,
                "Changing layout changed the selected article"
            );
            let reader = ui::workspace_areas(screen, app.focus, &app.workspace)
                .reader
                .ok_or_else(|| anyhow::anyhow!("Reader was not restored"))?;
            let content = ui::workspace_reader_content(reader, &app.workspace);
            app.resize_reader(content.width.into(), content.height.into());
            Ok(())
        },
    )
}

fn navigation_workload(
    store: Store,
    count: usize,
    iterations: usize,
) -> Result<()> {
    lume::catalog::install(&store)?;
    let mut app = App::new(store, false)?;
    app.focus = lume::app::Focus::Library;
    measure(
        "library down/up with 98 sources + 46 searches",
        iterations,
        || {
            app.key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE))?;
            ensure!(app.view == lume::model::View::All, "Wrong destination");
            app.key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE))?;
            ensure!(
                app.view == lume::model::View::Inbox,
                "Inbox was not restored"
            );
            ensure!(app.page_total == count, "Navigation lost unread articles");
            Ok(())
        },
    )
}

fn released_input(app: &mut App, iterations: usize) -> Result<()> {
    use crossterm::event::{Event, KeyEventKind};
    let repeat = Event::Key(KeyEvent::new_with_kind(
        KeyCode::Down,
        KeyModifiers::NONE,
        KeyEventKind::Repeat,
    ));
    let release = Event::Key(KeyEvent::new_with_kind(
        KeyCode::Down,
        KeyModifiers::NONE,
        KeyEventKind::Release,
    ));
    let selected = app.current().map(|article| article.id);
    measure("240 queued repeats + release + quit", iterations, || {
        let mut events = vec![repeat.clone(); 240];
        events.push(release.clone());
        events.push(Event::Key(KeyEvent::new(
            KeyCode::Char('q'),
            KeyModifiers::NONE,
        )));
        let mut batch = lume::app::InputBatch::new(events);
        ensure!(
            batch.dispatch_next(app)? == Some(lume::app::Action::None),
            "Repeat not consumed"
        );
        ensure!(
            batch.dispatch_next(app)? == Some(lume::app::Action::None),
            "Release not consumed"
        );
        ensure!(
            batch.dispatch_next(app)? == Some(lume::app::Action::Quit),
            "Quit was delayed"
        );
        ensure!(
            app.current().map(|article| article.id) == selected,
            "Released repeat moved selection"
        );
        Ok(())
    })
}
