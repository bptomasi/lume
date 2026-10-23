//! Ordered terminal bursts skip intermediate navigation loads and frames.
//!
//! Only normal-mode motion coalesces. Drafts, searches and picker text retain
//! every event. Reported releases cancel pending autorepeats; legacy terminal
//! motion is combined into one destination change without losing its distance.

use std::collections::VecDeque;

use anyhow::Result;
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent,
    MouseEventKind,
};

use super::{Action, App, Focus, Mode};

/// Ordered input collected before the next frame, without doing application I/O.
pub struct InputBatch {
    events: VecDeque<Event>,
}

impl InputBatch {
    /// Retain hover motion; the adapter bounds each batch to 256 events.
    pub fn new(events: Vec<Event>) -> Self {
        Self {
            events: events.into(),
        }
    }

    /// Whether a burst contains unprocessed events, including hover motion.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Whether events require a frame regardless of annotation-hover changes.
    pub fn needs_redraw(&self) -> bool {
        self.events.iter().any(|event| match event {
            Event::Mouse(mouse) if mouse.kind == MouseEventKind::Moved => false,
            Event::Key(key) => {
                key.kind != KeyEventKind::Release
                    && !matches!(key.code, KeyCode::Modifier(_))
            }
            _ => true,
        })
    }

    /// Dispatch the next action against the current mode, retaining event order.
    ///
    /// # Errors
    /// Returns application/storage failures. Remaining input is retained.
    pub fn dispatch_next(&mut self, app: &mut App) -> Result<Option<Action>> {
        let Some(event) = self.events.pop_front() else {
            return Ok(None);
        };
        let action = match event {
            Event::Key(key) => self.dispatch_key(app, key)?,
            Event::Paste(text) => {
                app.paste(&text)?;
                Action::None
            }
            Event::Mouse(mouse) => self.dispatch_mouse(app, mouse)?,
            Event::Resize(mut width, mut height) => {
                while let Some(Event::Resize(next_width, next_height)) =
                    self.events.front()
                {
                    (width, height) = (*next_width, *next_height);
                    self.events.pop_front();
                }
                app.resize_workspace(ratatui::layout::Rect::new(
                    0, 0, width, height,
                ));
                Action::None
            }
            Event::FocusLost => {
                app.cancel_pointer();
                app.note_marks.delete = None;
                app.pending_destination_delete = None;
                app.pending_article_delete = None;
                Action::None
            }
            _ => Action::None,
        };
        Ok(Some(action))
    }

    fn dispatch_mouse(
        &mut self,
        app: &mut App,
        mut mouse: MouseEvent,
    ) -> Result<Action> {
        let wheel = matches!(
            mouse.kind,
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
        );
        let motion = matches!(
            mouse.kind,
            MouseEventKind::Drag(_) | MouseEventKind::Moved
        );
        let mut amount: usize = 1;
        while let Some(Event::Mouse(next)) = self.events.front() {
            if next.kind != mouse.kind
                || next.modifiers != mouse.modifiers
                || !(motion
                    || wheel
                        && next.column == mouse.column
                        && next.row == mouse.row)
            {
                break;
            }
            mouse = *next;
            self.events.pop_front();
            amount += 1;
        }
        if wheel {
            app.pointer_scroll(mouse, amount)?;
            return Ok(Action::None);
        }
        app.mouse(mouse)
    }

    fn dispatch_key(&mut self, app: &mut App, key: KeyEvent) -> Result<Action> {
        if released_reader_repeat(app, key) && self.release_pending(key) {
            return Ok(Action::None);
        }
        let Some((forward, amount)) = motion(app, key) else {
            return app.key(key);
        };
        let released = self.release_pending(key);
        let mut steps: usize = 1;
        while let Some(Event::Key(next)) = self.events.front() {
            if !same_key(key, *next) || next.kind == KeyEventKind::Release {
                break;
            }
            self.events.pop_front();
            steps += 1;
        }
        if released {
            if key.kind == KeyEventKind::Repeat {
                return Ok(Action::None);
            }
            // A quick press/release still moves once. Its queued autorepeats
            // must not move the selection after the release has been observed.
            steps = 1;
        }
        app.cancel_pointer();
        app.pending_g = false;
        let distance = amount.saturating_mul(steps);
        if app.notebook.is_some() {
            app.move_notebook(forward, distance)?;
        } else {
            app.move_selection(forward, distance)?;
        }
        Ok(Action::None)
    }

    fn release_pending(&self, key: KeyEvent) -> bool {
        for event in &self.events {
            let Event::Key(next) = event else {
                continue;
            };
            if !same_key(key, *next) {
                continue;
            }
            match next.kind {
                KeyEventKind::Press => return false,
                KeyEventKind::Release => return true,
                KeyEventKind::Repeat => {}
            }
        }
        false
    }
}

fn released_reader_repeat(app: &App, key: KeyEvent) -> bool {
    app.focus == Focus::Reader
        && matches!(app.mode, Mode::Normal)
        && !app.vim_pending()
        && !app
            .workspace
            .keybindings
            .contains_key(&super::key_name(key))
        && key.kind == KeyEventKind::Repeat
        && key.modifiers.difference(KeyModifiers::SHIFT).is_empty()
        && matches!(
            key.code,
            KeyCode::Left
                | KeyCode::Right
                | KeyCode::Char('h' | 'l' | 'w' | 'W' | 'b' | 'B' | 'e' | 'E')
        )
}

fn same_key(first: KeyEvent, second: KeyEvent) -> bool {
    first.code == second.code && first.modifiers == second.modifiers
}

fn motion(app: &App, key: KeyEvent) -> Option<(bool, usize)> {
    if !matches!(app.mode, Mode::Normal)
        || app.vim_pending()
        || app
            .workspace
            .keybindings
            .contains_key(&super::key_name(key))
        || key.kind == KeyEventKind::Release
        || key.modifiers != KeyModifiers::NONE
    {
        return None;
    }
    match key.code {
        KeyCode::Down | KeyCode::Char('j') => Some((true, 1)),
        KeyCode::Up | KeyCode::Char('k') => Some((false, 1)),
        KeyCode::PageDown => Some((true, app.reader_height)),
        KeyCode::PageUp => Some((false, app.reader_height)),
        _ => None,
    }
}
