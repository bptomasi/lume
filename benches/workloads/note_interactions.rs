//! Cached hover frames and reversible note operators on real SQLite libraries.

use anyhow::{Context, Result, ensure};
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind,
};
use lume::{
    app::{Action, App, Focus, InputBatch, InputKind, Mode},
    model::{FeedOrigin, IncomingArticle},
    store::Store,
    ui,
};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};

const SCREEN: Rect = Rect::new(0, 0, 140, 36);

pub fn run(iterations: usize, stress: bool) -> Result<()> {
    let (directory, mut app) = application(stress)?;
    let store = Store::open(&directory.path().join("notes.sqlite3"))?;
    let mut terminal = Terminal::new(TestBackend::new(140, 36))?;
    println!(
        "note input: {} source bytes, {} annotations",
        app.body.len(),
        app.comments.len(),
    );
    hover(&mut app, &mut terminal, iterations)?;
    writes(&mut app, &mut terminal, iterations)?;
    card_writes(&mut app, &store, iterations)?;
    deletion(&mut app, iterations)
}

fn card_writes(app: &mut App, store: &Store, iterations: usize) -> Result<()> {
    let article = app.current().context("Missing article")?.id;
    let comment = app.comments[0].id;
    let mut answer = "The transaction owner".to_owned();
    command(app, "card")?;
    app.paste(&format!("Who writes? | {answer}"))?;
    app.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))?;
    command(app, "w")?;
    let card = store.annotation(article, comment)?.cards[0].clone();
    store.review_study_card(&card, 7, 100)?;
    super::measure(
        "card edit + :w + unchanged :w + persisted schedule",
        iterations,
        || {
            keys(app, "A.")?;
            answer.push('.');
            app.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))?;
            command(app, "w")?;
            command(app, "w")?;
            let actual = store.annotation(article, comment)?.cards;
            ensure!(
                actual.len() == 1 && actual[0].id == card.id,
                "Repeated :w duplicated or replaced a card"
            );
            ensure!(
                actual[0].question == "Who writes?"
                    && actual[0].answer == answer,
                "Card text changed"
            );
            ensure!(
                actual[0].due_at == 604_900 && actual[0].reviews == 1,
                "Writing the card lost its review schedule"
            );
            Ok(())
        },
    )?;
    command(app, "q")?;
    Ok(())
}

fn writes(
    app: &mut App,
    terminal: &mut Terminal<TestBackend>,
    iterations: usize,
) -> Result<()> {
    let id = app.comments[0].id;
    let count = app.comments.len();
    let mut expected = app.comments[0].body.clone();
    command(app, "edit-note")?;
    app.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))?;
    super::measure(
        "note edit + :w + unchanged :w + editor frame",
        iterations,
        || {
            keys(app, "A.")?;
            expected.push('.');
            app.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))?;
            command(app, "w")?;
            command(app, "w")?;
            terminal.draw(|frame| ui::draw(frame, app))?;
            ensure!(
                app.comments.len() == count,
                "Repeated :w duplicated a note"
            );
            ensure!(
                app.comments[0].id == id && app.comments[0].body == expected,
                "Repeated :w changed its captured identity or text"
            );
            ensure!(
                matches!(&app.mode, Mode::Input(input)
            if matches!(input.kind, InputKind::Comment { comment: Some(saved), .. }
                if saved == id) && !input.modified()),
                "Saved editor lost its identity or stayed dirty"
            );
            Ok(())
        },
    )?;
    command(app, "q")?;
    Ok(())
}

fn command(app: &mut App, text: &str) -> Result<()> {
    keys(app, ":")?;
    app.paste(text)?;
    ensure!(
        app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?
            == Action::None,
        "Unexpected exit"
    );
    Ok(())
}

fn keys(app: &mut App, text: &str) -> Result<()> {
    for character in text.chars() {
        app.key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE))?;
    }
    Ok(())
}

fn hover(
    app: &mut App,
    terminal: &mut Terminal<TestBackend>,
    iterations: usize,
) -> Result<()> {
    let column = ui::workspace_reader_content(SCREEN, &app.workspace);
    let event = MouseEvent {
        kind: MouseEventKind::Moved,
        column: column.x - 1,
        row: column.y,
        modifiers: KeyModifiers::NONE,
    };
    let id = app.comments[0].id;
    super::measure(
        "note hover + popup + dismissal frames",
        iterations * 10,
        || {
            let mut batch = InputBatch::new(vec![Event::Mouse(event); 64]);
            ensure!(!batch.needs_redraw(), "Idle motion forces frames");
            batch.dispatch_next(app)?;
            ensure!(batch.is_empty(), "Hover burst did not coalesce");
            ensure!(
                app.hovered_note().map(|(id, _)| id) == Some(id),
                "Hover chose a different annotation"
            );
            terminal.draw(|frame| ui::draw(frame, app))?;
            app.mouse(MouseEvent {
                column: 0,
                row: 0,
                ..event
            })?;
            ensure!(app.hovered_note().is_none(), "Preview stayed open");
            terminal.draw(|frame| ui::draw(frame, app))?;
            Ok(())
        },
    )
}

fn deletion(app: &mut App, iterations: usize) -> Result<()> {
    let count = app.comments.len();
    let id = app.comments[0].id;
    super::measure(
        "contextual note dd + undo + restored marker",
        iterations,
        || {
            app.reader_cursor = 0;
            app.reader_column = 2;
            for character in "dd".chars() {
                app.key(KeyEvent::new(
                    KeyCode::Char(character),
                    KeyModifiers::NONE,
                ))?;
            }
            ensure!(app.comments.len() == count - 1, "Wrong deletion count");
            ensure!(
                !app.comments.iter().any(|note| note.id == id),
                "Wrong note was deleted"
            );
            app.key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::NONE))?;
            ensure!(app.comments.len() == count, "Undo lost an annotation");
            ensure!(
                app.reader_note_markers(0).contains(&id),
                "Restored annotation lost its marker"
            );
            Ok(())
        },
    )
}

fn application(stress: bool) -> Result<(tempfile::TempDir, App)> {
    let directory = tempfile::tempdir()?;
    let mut store = Store::open(&directory.path().join("notes.sqlite3"))?;
    let id = store.subscribe("https://benchmark.example/notes")?;
    let feed = store
        .feeds()?
        .into_iter()
        .find(|feed| feed.id == id)
        .context("Feed missing")?;
    let paragraphs = if stress { 1000 } else { 200 };
    let count = if stress { 200 } else { 20 };
    let body = (0..paragraphs)
        .map(|index| {
            format!("A café 🦀 studies distributed systems {index}.\n\n")
        })
        .collect::<String>();
    store.ingest(&feed, "Systems", &[incoming(&body)])?;
    let article = store.library()?.articles[0].id;
    for (anchor, _) in body.match_indices("café").take(count) {
        store.save_highlight(
            article,
            "café",
            anchor,
            "Review this mechanism",
        )?;
    }
    let mut app = App::new(store, false)?;
    app.focus = Focus::Reader;
    app.workspace.fullscreen = true;
    app.resize_workspace(SCREEN);
    Ok((directory, app))
}

fn incoming(body: &str) -> IncomingArticle {
    IncomingArticle {
        body_origin: FeedOrigin::Feed,
        external_id: "notes".into(),
        title: "Distributed systems".into(),
        url: "https://benchmark.example/notes/article".into(),
        author: "Author".into(),
        published: 1_700_000_000,
        body: body.into(),
    }
}
