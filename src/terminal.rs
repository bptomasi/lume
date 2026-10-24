//! Event-driven runtime with terminal restoration on errors and panics.

mod background;
mod images;
mod input;
mod presentation;
mod welcome;

use background::Background;

use std::{
    io::{self, IsTerminal},
    path::PathBuf,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use crossterm::{
    SynchronizedUpdate,
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
    configuration: Option<lume::config::Session>,
) -> Result<()> {
    if !io::stdout().is_terminal() || !io::stdin().is_terminal() {
        bail!(
            "Open Lume in an interactive terminal; use --help for CLI commands"
        );
    }
    let mut terminal = TerminalSession::start()?;
    let workspace = store.workspace()?;
    terminal
        .terminal
        .draw(|frame| ui::draw_welcome(frame, &workspace.theme))?;
    let mut app = App::open_interactive(store, demo)?;
    if let Some(session) = &configuration {
        app.configure_document(session.initial().clone());
    }
    run_session(&mut app, path, demo, offline, configuration, &mut terminal)
}

fn run_session(
    app: &mut App,
    path: PathBuf,
    demo: bool,
    offline: bool,
    configuration: Option<lume::config::Session>,
    terminal: &mut TerminalSession,
) -> Result<()> {
    let mut background = None;
    let result = (|| {
        app.enable_background_navigation(&path)?;
        app.set_network_enabled(!demo && !offline);
        app.load_workspace();
        images::start(app, &path, offline || demo);
        let worker = Background::start(
            path,
            !demo && !offline,
            configuration
                .as_ref()
                .map(lume::config::Session::start)
                .transpose()?,
        )?;
        let worker = background.insert(worker);
        if !offline
            && (!app.library.feeds.is_empty()
                || app.library.servers.iter().any(|server| server.enabled))
        {
            worker.refresh(app);
        }
        run_terminal(app, worker, terminal)
    })();
    finalize_runtime(app, result, background.as_mut(), configuration.as_ref())
}

fn finalize_runtime(
    app: &mut App,
    result: Result<()>,
    background: Option<&mut Background>,
    configuration: Option<&lume::config::Session>,
) -> Result<()> {
    let saved = app.end_session();
    let published = match background {
        Some(background) => background.finish_configuration(),
        None => configuration.map_or(Ok(()), lume::config::Session::flush),
    };
    finish_session([result, saved, published])
}

fn finish_session(results: [Result<()>; 3]) -> Result<()> {
    let failures = results
        .into_iter()
        .filter_map(Result::err)
        .map(|error| format!("{error:#}"))
        .collect::<Vec<_>>();
    anyhow::ensure!(failures.is_empty(), "{}", failures.join("; "));
    Ok(())
}

struct TerminalSession {
    terminal: ratatui::DefaultTerminal,
    guard: TerminalGuard,
}

impl TerminalSession {
    fn start() -> Result<Self> {
        let guard = TerminalGuard {
            keyboard: false,
            mouse: input::MouseCapture::default(),
        };
        let terminal =
            ratatui::try_init().context("Could not initialize terminal")?;
        Ok(Self { terminal, guard })
    }
}

fn run_terminal(
    app: &mut App,
    background: &mut Background,
    session: &mut TerminalSession,
) -> Result<()> {
    if matches!(
        crossterm::terminal::supports_keyboard_enhancement(),
        Ok(true)
    ) {
        input::enable_keyboard()?;
        session.guard.keyboard = true;
    }
    execute!(io::stdout(), EnableBracketedPaste, EnableFocusChange)?;
    event_loop(&mut session.terminal, app, background, &mut session.guard)
}

fn event_loop(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
    background: &mut Background,
    guard: &mut TerminalGuard,
) -> Result<()> {
    let mut dirty = true;
    let mut welcome = true;
    let mut presentation = presentation::Presentation::default();
    loop {
        if !welcome {
            dirty |= sync_mouse(guard, app);
            dirty |= poll_auto_read(app);
        }
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
            draw_workspace(terminal, app, welcome, &mut presentation)?;
            dirty = false;
        }
        if !event::poll(poll_interval(app))? {
            continue;
        }
        match dispatch_input(terminal, app, background, guard, &mut welcome)? {
            InputOutcome::Quit => break,
            InputOutcome::Redraw => dirty = true,
            InputOutcome::Idle => {}
        }
    }
    Ok(())
}

enum InputOutcome {
    Idle,
    Redraw,
    Quit,
}

fn dispatch_input(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
    background: &Background,
    guard: &mut TerminalGuard,
    welcome: &mut bool,
) -> Result<InputOutcome> {
    if *welcome {
        return Ok(welcome::handle(welcome, event::read()?));
    }
    let mut batch = input::read_batch()?;
    let actionable = batch.needs_redraw();
    let hover = app.hovered_note();
    if dispatch_batch(&mut batch, terminal, app, background, guard)? {
        return Ok(InputOutcome::Quit);
    }
    if actionable || hover != app.hovered_note() {
        return Ok(InputOutcome::Redraw);
    }
    Ok(InputOutcome::Idle)
}

fn poll_auto_read(app: &mut App) -> bool {
    match app.poll_auto_read(Instant::now()) {
        Ok(changed) => changed,
        Err(error) => {
            app.status = format!("{error:#}");
            true
        }
    }
}

fn poll_reading_workers(app: &mut App) -> bool {
    let mut changed = app.poll_reader();
    changed |= app.poll_images();
    for result in [
        app.poll_navigation(),
        app.poll_recommendations(),
        app.poll_picker(),
        app.poll_web(),
        app.poll_maintenance(),
    ] {
        match result {
            Ok(ready) => changed |= ready,
            Err(error) => {
                app.status = format!("{error:#}");
                changed = true;
            }
        }
    }
    if let Some(address) = app.take_web_browser_request() {
        launch_browser(app, &address);
        changed = true;
    }
    if !app.article_preparing() && !app.reader_stale() {
        app.finish_measurement(lume::app::Operation::Article);
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
        || app.picker_pending()
        || app.web_pending()
        || app.maintenance_pending()
        || app.navigation_loading
        || app.catalog_pending()
        || app.pane_motion_active();
    Duration::from_millis(if active { 8 } else { 100 })
}

fn draw_workspace(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
    welcome: bool,
    presentation: &mut presentation::Presentation,
) -> Result<()> {
    app.begin_measurement(lume::app::Operation::Frame);
    if welcome {
        io::stdout().sync_update(|_| {
            terminal.draw(|frame| ui::draw_welcome(frame, &app.workspace.theme))
        })??;
        app.finish_measurement(lume::app::Operation::Frame);
        return Ok(());
    }
    let area = terminal.size()?;
    let full = ratatui::layout::Rect::new(0, 0, area.width, area.height);
    let pixels = images::pixels();
    app.update_image_cell_pixels(pixels);
    app.resize_workspace(full);
    let native_inspector = app.uses_native_image_inspector();
    let sixel = app.uses_sixel_images();
    let mut draw = || {
        presentation.draw(terminal, native_inspector, |frame| {
            ui::draw_interactive(frame, app)
        })
    };
    if sixel {
        io::stdout().sync_update(|_| draw())??;
    } else {
        draw()?;
    }
    app.finish_measurement(lume::app::Operation::Frame);
    Ok(())
}

fn dispatch_batch(
    batch: &mut InputBatch,
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
    background: &Background,
    guard: &mut TerminalGuard,
) -> Result<bool> {
    while !batch.is_empty() {
        app.begin_measurement(lume::app::Operation::Input);
        let result = batch.dispatch_next(app);
        app.finish_measurement(lume::app::Operation::Input);
        let action = match result {
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
            Action::ReloadConfiguration => background.reload_configuration(app),
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
                        app.clear_copied_selection();
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
    Ok(false)
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
        let _ = execute!(
            io::stdout(),
            crossterm::terminal::EndSynchronizedUpdate,
            DisableBracketedPaste,
            DisableFocusChange,
        );
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

#[cfg(test)]
mod durability_tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    #[test]
    fn drawing_and_input_failures_still_drain_personal_writes_and_recovery()
    -> Result<()> {
        for failure in ["injected draw failure", "injected input failure"] {
            let directory = tempfile::tempdir()?;
            let path = directory.path().join("library.sqlite3");
            let mut store = Store::open(&path)?;
            lume::demo::seed(&mut store)?;
            let mut app = App::new(store, false)?;
            let article = app.current().context("Demo article")?;
            let id = article.id;
            let favorite = !article.starred;
            app.enable_background_navigation(&path)?;
            app.key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE))?;
            app.focus = lume::app::Focus::Reader;
            app.key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE))?;
            app.paste("Survive terminal I/O failure")?;

            let result = finalize_runtime(
                &mut app,
                Err(anyhow::anyhow!(failure)),
                None,
                None,
            );

            assert!(
                result
                    .err()
                    .context("Expected runtime failure")?
                    .to_string()
                    .contains(failure)
            );
            let store = Store::open(&path)?;
            assert_eq!(store.article(id)?.starred, favorite);
            assert_eq!(store.note_draft_count()?, 1);
        }
        Ok(())
    }
}
