//! Export real Ratatui buffer cells for deterministic visual review.

mod support;

use std::{
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, ensure};
use lume::{app::App, demo, store::Store, ui};
use ratatui::{Terminal, backend::TestBackend};

fn main() -> Result<()> {
    let output = std::env::args_os()
        .nth(1)
        .map_or_else(|| PathBuf::from("/tmp/lume-preview.json"), PathBuf::from);
    let mode = std::env::args().nth(2).unwrap_or_default();
    let (_directory, mut app) = demo_app(&mode)?;
    finish_colors(&mut app)?;
    finish_notebook(&mut app)?;
    if mode == "note-hover" {
        preview_note_hover(&mut app)?;
    } else if mode == "note-write" {
        preview_note_editor(&mut app)?;
    }
    let mut terminal = Terminal::new(TestBackend::new(144, 44))?;
    terminal.draw(|frame| ui::draw_interactive(frame, &mut app))?;
    support::export(terminal.backend().buffer(), &output)?;
    println!("{}", output.display());
    Ok(())
}

fn preview_note_editor(app: &mut App) -> Result<()> {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    app.focus = lume::app::Focus::Reader;
    app.key(KeyEvent::new(KeyCode::Char(':'), KeyModifiers::NONE))?;
    app.paste("note")?;
    app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?;
    app.paste(
        "One owner per transaction.\n\n\
         Keep retries bounded and preserve the reader's position.\n\n\
         Compare this with the consensus paper before the next review.",
    )?;
    app.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))?;
    app.key(KeyEvent::new(KeyCode::Char(':'), KeyModifiers::NONE))?;
    app.paste("wq")
}

fn finish_colors(app: &mut App) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(10);
    while app.reader_pending() && Instant::now() < deadline {
        app.poll_reader();
        thread::sleep(Duration::from_millis(1));
    }
    ensure!(!app.reader_pending(), "Preview syntax worker timed out");
    Ok(())
}

fn finish_notebook(app: &mut App) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(10);
    while app.notebook_pending() && Instant::now() < deadline {
        app.poll_note_workspace()?;
        thread::sleep(Duration::from_millis(1));
    }
    ensure!(!app.notebook_pending(), "Preview notebook worker timed out");
    Ok(())
}

fn preview_note_hover(app: &mut App) -> Result<()> {
    use crossterm::event::{KeyModifiers, MouseEvent, MouseEventKind};
    let screen = ratatui::layout::Rect::new(0, 0, 144, 44);
    let area = ui::workspace_areas(screen, app.focus, &app.workspace)
        .reader
        .context("Missing reader")?;
    let column = ui::workspace_reader_content(area, &app.workspace);
    let row = (0..app.reader_lines.len())
        .find(|row| !app.reader_note_markers(*row).is_empty())
        .context("Missing note marker")?;
    app.scroll = row;
    app.mouse(MouseEvent {
        kind: MouseEventKind::Moved,
        column: column.x - 1,
        row: column.y,
        modifiers: KeyModifiers::NONE,
    })?;
    Ok(())
}

fn demo_app(mode: &str) -> Result<(tempfile::TempDir, App)> {
    let directory = tempfile::tempdir()?;
    let mut store = Store::open(&directory.path().join("demo.sqlite3"))?;
    seed_preview(&mut store, mode)?;
    let mut app = App::new(store, true)?;
    if mode == "fetching"
        && let Some(article) = app.current()
    {
        app.article_fetch_started(lume::model::ArticleTarget {
            article: article.id,
            url: article.url.clone(),
        });
    }
    let keys = match mode {
        "zen" => " z",
        "fullscreen" | "code" | "partial" | "full-article" | "fetching" => "Z",
        "selection" => "Zjjjjjjv$sjj0llllvlllll",
        "cursor" => "Zjjj0lllll",
        "palette" => " p",
        "commands" => ":",
        "notebook" => " n",
        "recommendations" | "recommend-why" => " R",
        _ => "",
    };
    for character in keys.chars() {
        app.key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char(character),
            crossterm::event::KeyModifiers::NONE,
        ))?;
    }
    configure_preview(&mut app, mode)?;
    app.resize_workspace(ratatui::layout::Rect::new(0, 0, 144, 44));
    if mode == "reordering" {
        preview_panel_move(&mut app)?;
    }
    Ok((directory, app))
}

fn seed_partial(store: &mut Store) -> Result<()> {
    let feed = store.feeds()?.remove(0);
    store.ingest(
        &feed,
        "Community feed",
        &[lume::model::IncomingArticle {
            body_origin: lume::model::FeedOrigin::Feed,
            external_id: "partial-preview".into(),
            title: "An interesting article discovered through a community"
                .into(),
            url: "https://author.example/article".into(),
            author: "Lume".into(),
            published: 1_800_000_000,
            body: "[Comments](<https://community.example/discussion>)".into(),
        }],
    )?;
    Ok(())
}

fn seed_preview(store: &mut Store, mode: &str) -> Result<()> {
    demo::seed(store)?;
    if matches!(
        mode,
        "annotations" | "reordering" | "note-hover" | "notebook"
    ) {
        seed_annotations(store)?;
    }
    if matches!(mode, "code" | "selection") {
        seed_code(store)?;
    }
    if matches!(
        mode,
        "partial" | "full-article" | "fetching" | "outline" | "tables"
    ) {
        seed_partial(store)?;
    }
    if matches!(mode, "full-article" | "outline" | "tables") {
        seed_full_article(store)?;
    }
    if matches!(mode, "recommendations" | "recommend-why") {
        loop {
            if !store.refresh_recommendations()?.remaining {
                break;
            }
        }
    }
    Ok(())
}

fn configure_preview(app: &mut App, mode: &str) -> Result<()> {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let commands: &[&str] = match mode {
        "study" | "layout-guide" => {
            &["workspace load Study", "panel-focus reader"]
        }
        "settings" => &["settings"],
        "recommend-why" => &["recommend-why"],
        "outline" => &["toc"],
        "tables" => &["tables"],
        "help" => &["help layout"],
        "annotations" | "reordering" | "note-hover" => {
            &["articles", "annotations"]
        }
        "collapsed" => &["panel library", "panel articles", "panel reader"],
        _ => return Ok(()),
    };
    for command in commands {
        app.key(KeyEvent::new(KeyCode::Char(':'), KeyModifiers::NONE))?;
        app.paste(command)?;
        app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?;
    }
    if mode == "layout-guide" {
        app.key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL))?;
    }
    if mode == "settings" {
        app.key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE))?;
    }
    Ok(())
}

fn seed_annotations(store: &Store) -> Result<()> {
    let article = store.library()?.articles.remove(0);
    let body = store.body(article.id)?;
    let passages = [
        (
            "Draw when something changes",
            "Measure idle redraws as well.",
        ),
        (
            "Moving the cursor should not",
            "Keep navigation work bounded.",
        ),
        (
            "A reader spends most of its life",
            "A quiet interface leaves room to think.",
        ),
    ];
    for (quote, note) in passages {
        store.save_highlight(
            article.id,
            quote,
            body.find(quote).context("Missing preview passage")?,
            note,
        )?;
    }
    Ok(())
}

fn seed_full_article(store: &Store) -> Result<()> {
    let article = store.library()?.articles.remove(0);
    let document = lume::source::article::extract(
        include_str!("../tests/fixtures/article.html"),
        &article.url,
    )?;
    store.cache_document(
        &lume::model::ArticleTarget {
            article: article.id,
            url: article.url,
        },
        &document,
    )?;
    Ok(())
}

fn seed_code(store: &mut Store) -> Result<()> {
    let feed = store.feeds()?.remove(0);
    store.ingest(
        &feed,
        "Lume language tour",
        &[lume::model::IncomingArticle {
            body_origin: lume::model::FeedOrigin::Feed,
            external_id: "code-tour".into(),
            title: "One reader, many languages".into(),
            url: "https://preview.example/code".into(),
            author: "Lume".into(),
            published: 1_800_000_000,
            body: include_str!("fixtures/code-tour.md").into(),
        }],
    )?;
    Ok(())
}

fn preview_panel_move(app: &mut App) -> Result<()> {
    use crossterm::event::{
        KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    };
    let areas = ui::workspace_areas(
        ratatui::layout::Rect::new(0, 0, 144, 44),
        app.focus,
        &app.workspace,
    );
    let grip = ui::pane_move_rect(areas.reader.context("Missing reader")?)
        .context("Missing move grip")?;
    let destination = areas.annotations.context("Missing annotations")?;
    app.mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: grip.x + 1,
        row: grip.y,
        modifiers: KeyModifiers::NONE,
    })?;
    app.mouse(MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: destination.x + 3,
        row: destination.y + 4,
        modifiers: KeyModifiers::NONE,
    })?;
    Ok(())
}
