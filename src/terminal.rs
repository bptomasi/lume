//! Event-driven runtime with terminal restoration on errors and panics.

mod background;
mod input;

use background::Background;

use std::{
    io::{self, IsTerminal},
    path::PathBuf,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use crossterm::{
    event::{
        self, DisableBracketedPaste, DisableFocusChange, EnableBracketedPaste,
        EnableFocusChange,
    },
    execute,
};
use lume::{
    app::{Action, App, InputBatch},
    store::Store,
    ui,
};

pub(super) fn run(
    store: Store,
    path: PathBuf,
    demo: bool,
    offline: bool,
) -> Result<()> {
    if !io::stdout().is_terminal() || !io::stdin().is_terminal() {
        bail!(
            "Open Lume in an interactive terminal; use --help for CLI commands"
        );
    }
    let mut app = App::new(store, demo)?;
    app.enable_background_navigation(&path)?;
    if let Some(directory) = path.parent() {
        app.enable_images(directory.join("images"), offline || demo)?;
    }
    let mut background = Background::start(path, !demo && !offline)?;
    if !offline && !app.library.feeds.is_empty() {
        background.refresh(&mut app);
    }
    let mut guard = TerminalGuard {
        keyboard: false,
        mouse: input::MouseCapture::default(),
    };
    let mut terminal =
        ratatui::try_init().context("Could not initialize terminal")?;
    if matches!(
        crossterm::terminal::supports_keyboard_enhancement(),
        Ok(true)
    ) {
        input::enable_keyboard()?;
        guard.keyboard = true;
    }
    execute!(io::stdout(), EnableBracketedPaste, EnableFocusChange)?;
    event_loop(&mut terminal, &mut app, &mut background, &mut guard)
}

fn event_loop(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
    background: &mut Background,
    guard: &mut TerminalGuard,
) -> Result<()> {
    let mut dirty = true;
    loop {
        dirty |= sync_mouse(guard, app);
        dirty |= background.tick(app)?;
        dirty |= poll_reading_workers(app);
        match app.poll_note_workspace() {
            Ok(Some(Action::Quit)) => break,
            Ok(Some(_)) => dirty = true,
            Ok(None) => {}
            Err(error) => {
                app.status = format!("{error:#}");
                dirty = true;
            }
        }
        dirty |= app.tick_pane_motion(Instant::now());
        if dirty {
            draw_workspace(terminal, app)?;
            dirty = false;
        }
        if !event::poll(poll_interval(app))? {
            continue;
        }
        let mut batch = input::read_batch()?;
        let actionable = batch.needs_redraw();
        let hover = app.hovered_note();
        if dispatch_batch(&mut batch, terminal, app, background, guard)? {
            break;
        }
        dirty |= actionable || hover != app.hovered_note();
    }
    app.end_session()
}

fn poll_reading_workers(app: &mut App) -> bool {
    let mut changed = app.poll_reader();
    changed |= app.poll_images();
    for result in [app.poll_navigation(), app.poll_recommendations()] {
        match result {
            Ok(ready) => changed |= ready,
            Err(error) => {
                app.status = format!("{error:#}");
                changed = true;
            }
        }
    }
    changed
}

fn sync_mouse(guard: &mut TerminalGuard, app: &mut App) -> bool {
    if let Err(error) = guard.mouse.sync(app.workspace.mouse) {
        app.status = format!(
            "Mouse capture unavailable: {error} · keyboard remains available"
        );
        return true;
    }
    false
}

fn poll_interval(app: &App) -> Duration {
    let active = app.reader_pending()
        || app.images_pending()
        || app.notebook_pending()
        || app.note_inspection_pending()
        || app.note_catalog_pending()
        || app.notes_pending()
        || app.recommendations_pending()
        || app.navigation_loading
        || app.pane_motion_active();
    Duration::from_millis(if active { 16 } else { 100 })
}

fn draw_workspace(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
) -> Result<()> {
    let area = terminal.size()?;
    let full = ratatui::layout::Rect::new(0, 0, area.width, area.height);
    app.resize_workspace(full);
    terminal.draw(|frame| ui::draw_interactive(frame, app))?;
    Ok(())
}

fn dispatch_batch(
    batch: &mut InputBatch,
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
    background: &Background,
    guard: &mut TerminalGuard,
) -> Result<bool> {
    loop {
        let action = match batch.dispatch_next(app) {
            Ok(Some(action)) => action,
            Ok(None) => return Ok(false),
            Err(error) => {
                app.status = format!("{error:#}");
                continue;
            }
        };
        match action {
            Action::Quit => return Ok(true),
            Action::Refresh => background.refresh(app),
            Action::Open(url) => launch_browser(app, &url),
            Action::Discover(url) => background.discover(app, url),
            Action::FetchArticle(target) => background.fetch(app, target),
            Action::PreviewRule(request) => {
                background.preview_rule(app, request)
            }
            Action::ExternalEditor => edit_external(terminal, app, guard)?,
            Action::Copy(text) => {
                match lume::clipboard::copy(&mut io::stdout().lock(), &text) {
                    Ok(()) => {
                        app.selection = None;
                        app.status = format!(
                            "Yank sent to terminal clipboard · {} bytes",
                            text.len(),
                        );
                    }
                    Err(error) => app.status = format!("Clipboard: {error:#}"),
                }
            }
            Action::None => {}
        }
    }
}

fn launch_browser(app: &mut App, address: &str) {
    let result = lume::source::link_url(address).and_then(|url| {
        let launcher = if cfg!(target_os = "macos") {
            "open"
        } else {
            "xdg-open"
        };
        let mut child = std::process::Command::new(launcher)
            .arg(url.as_str())
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .context("Could not start the system browser")?;
        std::thread::Builder::new()
            .name("lume-browser".into())
            .spawn(move || {
                let _ = child.wait();
            })
            .context("Could not launch browser")?;
        Ok(())
    });
    app.status = match result {
        Ok(()) => "Original URL sent to the system browser".into(),
        Err(error) => error.to_string(),
    };
}

struct TerminalGuard {
    keyboard: bool,
    mouse: input::MouseCapture,
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = self.mouse.suspend();
        if self.keyboard {
            let _ = input::disable_keyboard();
        }
        let _ =
            execute!(io::stdout(), DisableBracketedPaste, DisableFocusChange);
        ratatui::restore();
    }
}

fn edit_external(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
    guard: &mut TerminalGuard,
) -> Result<()> {
    let Some(draft) = app.external_draft() else {
        return Ok(());
    };
    let command = std::env::var("VISUAL")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var("EDITOR").ok());
    let Some(command) = command else {
        app.status = "Set VISUAL or EDITOR to use an external editor".into();
        return Ok(());
    };
    let enhanced = guard.keyboard;
    guard.mouse.suspend()?;
    if enhanced {
        input::disable_keyboard()?;
        guard.keyboard = false;
    }
    execute!(io::stdout(), DisableBracketedPaste, DisableFocusChange)?;
    ratatui::restore();
    let result = lume::external_editor::edit(draft, &command);
    *terminal = ratatui::try_init().context("Could not resume terminal")?;
    if enhanced {
        input::enable_keyboard()?;
        guard.keyboard = true;
    }
    execute!(io::stdout(), EnableBracketedPaste, EnableFocusChange)?;
    guard.mouse.sync(app.workspace.mouse)?;
    match result {
        Ok(text) => {
            app.replace_draft(text)?;
            app.status =
                "Draft updated · Ctrl-s saves/closes · Esc normal".into();
        }
        Err(error) => app.status = format!("{error:#}"),
    }
    Ok(())
}
