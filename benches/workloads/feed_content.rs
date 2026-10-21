//! Mixed partial feeds and cached reader notices, with independent outcomes.

use std::hint::black_box;

use anyhow::{Context, Result, ensure};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lume::{
    app::App,
    content::{self, BodyKind},
    source,
    store::Store,
    ui,
};
use ratatui::{Terminal, backend::TestBackend};

pub(super) fn run(iterations: usize, stress: bool) -> Result<()> {
    let copies = if stress { 400 } else { 40 };
    let fixture = fixture(copies)?;
    println!(
        "partial feed input · entries={} · bytes={}",
        copies * 5,
        fixture.len()
    );
    super::measure("mixed feed fallback + availability", iterations, || {
        let parsed = source::parse(
            black_box(fixture.as_bytes()),
            "https://journal.example/feed",
        )?;
        ensure!(parsed.articles.len() == copies * 5, "Entries lost");
        for chunk in parsed.articles.as_chunks::<5>().0 {
            ensure!(
                chunk[0].body == "A useful short summary."
                    && chunk[1].body == "A **formatted** summary."
                    && chunk[2].body == "Safe fallback text.",
                "Empty content hid a publisher summary"
            );
            ensure!(
                content::body_kind(&chunk[3].body) == BodyKind::ReferencesOnly
                    && content::body_kind(&chunk[4].body) == BodyKind::Missing,
                "Availability classification changed"
            );
        }
        Ok(())
    })?;
    notices(iterations, stress)?;
    archives(iterations, stress)
}

pub(super) fn archives(iterations: usize, stress: bool) -> Result<()> {
    let entries = if stress { 2000 } else { 200 };
    let repetitions = if stress { 384 } else { 1536 };
    let payload = "Archive payload. ".repeat(repetitions);
    let mut fixture = String::from(
        "<rss version=\"2.0\"><channel><title>Archive Journal</title>\n",
    );
    for index in 0..entries {
        fixture.push_str(&format!(
            r#"<item>
    <guid isPermaLink="false">archive-{index}</guid>
    <title>Archive article {index}</title>
    <description><![CDATA[<p>{payload}</p>]]></description>
</item>
"#
        ));
    }
    fixture.push_str("</channel></rss>\n");
    println!(
        "archive feed input · entries={entries} · bytes={}",
        fixture.len()
    );
    super::measure("archive RSS decode + normalization", iterations, || {
        let parsed = source::parse(
            black_box(fixture.as_bytes()),
            "https://journal.example/feed",
        )?;
        ensure!(parsed.title == "Archive Journal", "Source title changed");
        ensure!(parsed.articles.len() == entries, "Archive entries lost");
        for (index, article) in parsed.articles.iter().enumerate() {
            ensure!(
                article.external_id == format!("archive-{index}")
                    && article.title == format!("Archive article {index}")
                    && article.body == payload.trim(),
                "Archive identity or content changed"
            );
        }
        Ok(())
    })
}

fn fixture(copies: usize) -> Result<String> {
    let template = include_str!("../../tests/fixtures/partial-content.xml");
    // Repeat a fixed owned fixture, not arbitrary publisher XML. The parser
    // remains feed-rs; these delimiters only assemble benchmark input.
    let start = template.find("<item>").context("Missing fixture entries")?;
    let end = template.rfind("</item>").context("Missing final entry")?
        + "</item>".len();
    let mut fixture = template[..start].to_owned();
    for index in 0..copies {
        fixture.push_str(&template[start..end].replace(
            "isPermaLink=\"false\">",
            &format!("isPermaLink=\"false\">batch-{index}-"),
        ));
    }
    fixture.push_str(&template[end..]);
    Ok(fixture)
}

fn notices(iterations: usize, stress: bool) -> Result<()> {
    let references = "[Comments](https://community.example/discussion)\n\n"
        .repeat(if stress { 2000 } else { 200 });
    let directory = tempfile::tempdir()?;
    let mut store = Store::open(&directory.path().join("notices.sqlite3"))?;
    let id = store.subscribe("https://journal.example/feed")?;
    let feed = store.feeds()?.remove(0);
    ensure!(feed.id == id, "Subscription changed");
    let mut articles = source::parse(
        include_bytes!("../../tests/fixtures/partial-content.xml"),
        &feed.url,
    )?
    .articles;
    articles[3].body = references;
    store.ingest(&feed, "Partial content journal", &articles)?;
    let mut app = App::new(store, false)?;
    app.resize_reader(72, 29);
    let mut terminal = Terminal::new(TestBackend::new(144, 44))?;
    super::measure(
        "partial reader selection + cached frame",
        iterations,
        || {
            for _ in 0..5 {
                terminal.draw(|frame| ui::draw(frame, &app))?;
                let notice = app.reader_text.contains("This feed supplied");
                ensure!(
                    notice == (app.body_kind != BodyKind::Text),
                    "Reader notice mismatches cached content"
                );
                let code = if app.selected == 4 {
                    KeyCode::Home
                } else {
                    KeyCode::Down
                };
                app.key(KeyEvent::new(code, KeyModifiers::NONE))?;
            }
            Ok(())
        },
    )
}
