//! Article cards keep stable pointer targets until selection leaves the window.

mod common;

use anyhow::{Context, Result};
use crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use lume::{
    app::App,
    model::{FeedOrigin, IncomingArticle, Pane, View},
    ui,
};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};

const SCREEN: Rect = Rect::new(0, 0, 160, 36);

fn articles(compact: bool) -> Result<(tempfile::TempDir, App)> {
    let (directory, mut store, feed) = common::seeded_store()?;
    let articles = (0..60)
        .map(|index| IncomingArticle {
            external_id: format!("article-{index}"),
            title: format!("Article {index:02}"),
            url: format!("https://journal.example/article-{index}"),
            author: "Fixture".into(),
            body: "Stable source text.".into(),
            body_origin: FeedOrigin::Feed,
            published: 1_900_000_000 + index,
        })
        .collect::<Vec<_>>();
    store.ingest(&feed, &feed.title, &articles)?;
    let mut app = App::new(store, false)?;
    app.workspace.compact = compact;
    app.focus = Pane::Articles;
    app.set_view(View::All)?;
    app.resize_workspace(SCREEN);
    Ok((directory, app))
}

fn area(app: &App) -> Result<Rect> {
    ui::workspace_areas(SCREEN, app.focus, &app.workspace)
        .articles
        .context("Articles hidden")
}

fn targets(app: &App, area: Rect) -> Vec<Option<usize>> {
    (area.y + 1..area.bottom() - 1)
        .map(|row| ui::article_at(app, area, row))
        .collect()
}

fn key(app: &mut App, code: KeyCode) -> Result<()> {
    app.focus = Pane::Articles;
    app.key(KeyEvent::new(code, KeyModifiers::NONE))?;
    app.resize_workspace(SCREEN);
    Ok(())
}

fn click(app: &mut App, area: Rect, local: usize) -> Result<()> {
    app.mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: area.x + 3,
        row: area.y + 1 + local as u16,
        modifiers: KeyModifiers::NONE,
    })?;
    Ok(())
}

#[test]
fn repeated_visible_clicks_keep_card_positions_and_pointer_targets()
-> Result<()> {
    for compact in [false, true] {
        let (_directory, mut app) = articles(compact)?;
        let area = area(&app)?;
        let before = targets(&app, area);
        let row_height = if compact { 2 } else { 4 };
        let bottom = before
            .iter()
            .rposition(Option::is_some)
            .context("No article cards")?
            / row_height
            * row_height;
        let middle = bottom / 2 / row_height * row_height;
        for local in [bottom, 0, middle, middle] {
            let expected = before[local].context("Card missing")?;
            click(&mut app, area, local)?;
            assert_eq!(app.selected, expected);
            assert_eq!(targets(&app, area), before, "Click moved the cards");
            let mut terminal =
                Terminal::new(TestBackend::new(SCREEN.width, SCREEN.height))?;
            terminal.draw(|frame| ui::draw(frame, &app))?;
            let id = app.visible[expected];
            let text = (area.x + 1..area.right() - 1)
                .map(|x| {
                    terminal.backend().buffer()[(x, area.y + 1 + local as u16)]
                        .symbol()
                })
                .collect::<String>();
            assert!(text.contains(&app.library.articles[id].title));
        }
    }
    Ok(())
}

#[test]
fn keyboard_scrolls_only_at_edges_and_keeps_the_new_window_when_reversing()
-> Result<()> {
    let (_directory, mut app) = articles(false)?;
    let area = area(&app)?;
    let first = targets(&app, area);
    let last = first.iter().flatten().copied().max().context("No cards")?;
    for _ in 0..last {
        key(&mut app, KeyCode::Char('j'))?;
        assert_eq!(targets(&app, area), first);
    }
    key(&mut app, KeyCode::Char('j'))?;
    let shifted = targets(&app, area);
    assert_eq!(shifted[0], Some(1));
    key(&mut app, KeyCode::Char('k'))?;
    assert_eq!(targets(&app, area), shifted);
    Ok(())
}

#[test]
fn background_article_delivery_preserves_the_clicked_card_window() -> Result<()>
{
    let (directory, mut app) = articles(false)?;
    app.enable_background_navigation(
        &directory.path().join("library.sqlite3"),
    )?;
    let area = area(&app)?;
    let before = targets(&app, area);
    for local in [20, 4, 16, 0] {
        click(&mut app, area, local)?;
        common::settle_notes(&mut app)?;
        assert_eq!(app.selected, before[local].context("Missing card")?);
        assert_eq!(targets(&app, area), before);
    }
    app.end_session()?;
    Ok(())
}

#[test]
fn new_pages_and_queries_do_not_reuse_an_out_of_range_window() -> Result<()> {
    let (_directory, mut app) = articles(false)?;
    app.page_size = 8;
    app.set_view(View::All)?;
    for _ in 0..7 {
        key(&mut app, KeyCode::Char('j'))?;
    }
    assert_eq!(targets(&app, area(&app)?)[0], Some(1));
    key(&mut app, KeyCode::Char('j'))?;
    assert_eq!((app.page_offset, app.selected), (8, 0));
    assert_eq!(targets(&app, area(&app)?)[0], Some(0));
    key(&mut app, KeyCode::Char('k'))?;
    assert_eq!((app.page_offset, app.selected), (0, 7));
    key(&mut app, KeyCode::Char('/'))?;
    app.paste("Article 59")?;
    app.resize_workspace(SCREEN);
    assert_eq!(app.selected, 0);
    let targets = targets(&app, area(&app)?);
    assert_eq!(targets[0], None, "Search field must not select a card");
    assert_eq!(targets[2], Some(0));
    Ok(())
}
