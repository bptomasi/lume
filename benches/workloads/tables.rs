//! Table preparation and cached frames keep Unicode, source and styles intact.

use anyhow::{Result, ensure};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lume::{
    app::{App, Focus},
    model::IncomingArticle,
    reader,
    store::Store,
    ui,
};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};

pub(super) fn run(iterations: usize, stress: bool) -> Result<()> {
    let (rows, columns) = match stress {
        true => (1000, 12),
        false => (64, 4),
    };
    let body = fixture(rows, columns);
    println!(
        "table input · rows={rows} · columns={columns} · bytes={}",
        body.len()
    );
    layout(&body, rows, iterations)?;
    publisher_header(rows, columns, iterations)?;
    frames(&body, rows, columns, iterations)
}

fn publisher_header(
    rows: usize,
    columns: usize,
    iterations: usize,
) -> Result<()> {
    let mut html = String::from("<table><tbody><tr>");
    for column in 0..columns {
        html.push_str(&format!("<td><strong>Column {column}</strong></td>"));
    }
    html.push_str("</tr>");
    for row in 0..rows {
        html.push_str("<tr>");
        for column in 0..columns {
            html.push_str(&format!("<td>{row}-{column}</td>"));
        }
        html.push_str("</tr>");
    }
    html.push_str("</tbody></table>");
    super::measure("publisher HTML table + semantic header", iterations, || {
        let source = lume::content::article_markdown(&html, true, "");
        let rendered = reader::render(&source, 80);
        ensure!(rendered.table_rows.len() == 1, "Table lost");
        ensure!(
            rendered.table_rows[0].len() == rows + 4,
            "Header gained a blank row"
        );
        ensure!(
            rendered.lines[1].to_string().contains("Column 0"),
            "Header misplaced"
        );
        Ok(())
    })
}

fn layout(body: &str, rows: usize, iterations: usize) -> Result<()> {
    // Initialize the embedded grammar before timing table preparation.
    let _ = reader::render(body, 80);
    super::measure("table layout + source correspondence", iterations, || {
        let rendered = reader::render(body, 80);
        ensure!(
            rendered.table_rows.len() == 1,
            "Grid unexpectedly fell back"
        );
        let range = rendered.table_rows[0].clone();
        ensure!(range.len() == rows + 4, "Table rows lost");
        let width = rendered.lines[range.start].width();
        ensure!(
            rendered.lines[range]
                .iter()
                .all(|line| line.width() == width),
            "Misaligned grid"
        );
        let marker = format!("row-{:04}", rows - 1);
        let offset = rendered
            .text
            .find(&marker)
            .ok_or_else(|| anyhow::anyhow!("Last row lost"))?;
        let source = reader::to_source(
            &rendered.mappings,
            offset..offset + marker.len(),
        )
        .ok_or_else(|| anyhow::anyhow!("Missing source mapping"))?;
        ensure!(body.get(source) == Some(marker.as_str()), "Source drifted");
        Ok(())
    })
}

fn frames(
    body: &str,
    rows: usize,
    columns: usize,
    iterations: usize,
) -> Result<()> {
    let (_directory, mut app) = application(body)?;
    app.resize_workspace(Rect::new(0, 0, 144, 44));
    app.reader_cursor = app.table_rows[0].end - 2;
    key(&mut app, '$')?;
    let expected = format!("value-{}-{}", rows - 1, columns - 1);
    let mut terminal = Terminal::new(TestBackend::new(144, 44))?;
    super::measure(
        "table motion + selected cached frame",
        iterations * 25,
        || {
            key(&mut app, 'h')?;
            key(&mut app, 'l')?;
            app.selection = Some(app.cursor_offset().saturating_sub(5));
            terminal.draw(|frame| ui::draw_interactive(frame, &mut app))?;
            let text = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|cell| cell.symbol())
                .collect::<String>();
            ensure!(
                text.contains(&expected),
                "Last column was clipped incorrectly"
            );
            ensure!(app.body == body, "Navigation changed publisher source");
            app.selection = None;
            Ok(())
        },
    )?;
    inspector(&mut app, &expected, iterations)
}

fn inspector(app: &mut App, expected: &str, iterations: usize) -> Result<()> {
    let cursor = app.cursor_offset();
    super::measure("cached table inspector + close", iterations, || {
        for character in ":tables".chars() {
            key(app, character)?;
        }
        app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?;
        let lume::app::Mode::Document(document) = &app.mode else {
            anyhow::bail!("Table inspector did not open");
        };
        ensure!(
            document.lines.iter().any(|line| line.contains(expected)),
            "Table cells lost"
        );
        app.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))?;
        ensure!(
            app.cursor_offset() == cursor,
            "Inspector moved reader cursor"
        );
        Ok(())
    })
}

fn application(body: &str) -> Result<(tempfile::TempDir, App)> {
    let directory = tempfile::tempdir()?;
    let mut store = Store::open(&directory.path().join("tables.sqlite3"))?;
    store.subscribe("https://journal.example/feed")?;
    let feed = store.feeds()?.remove(0);
    store.ingest(
        &feed,
        "Tables",
        &[IncomingArticle {
            external_id: "comparison".into(),
            title: "Measured comparison".into(),
            url: "https://journal.example/comparison".into(),
            author: "Fixture".into(),
            published: 1_800_000_000,
            body: body.into(),
            body_origin: lume::model::FeedOrigin::Feed,
        }],
    )?;
    let mut app = App::new(store, false)?;
    app.focus = Focus::Reader;
    app.workspace.fullscreen = true;
    Ok((directory, app))
}

fn fixture(rows: usize, columns: usize) -> String {
    let header = (0..columns)
        .map(|column| format!("Column {column}"))
        .collect::<Vec<_>>()
        .join(" | ");
    let mut body =
        format!("| {header} |\n| {} |\n", vec!["---"; columns].join(" | "));
    for row in 0..rows {
        let mut values = vec![format!("row-{row:04}")];
        values.extend(
            (1..columns).map(|column| format!("界 value-{row}-{column}")),
        );
        body.push_str(&format!("| {} |\n", values.join(" | ")));
    }
    body.push_str("\n```rust\nlet n = 42;\n\nn\n```\n");
    body
}

fn key(app: &mut App, key: char) -> Result<()> {
    app.key(KeyEvent::new(KeyCode::Char(key), KeyModifiers::NONE))?;
    Ok(())
}
