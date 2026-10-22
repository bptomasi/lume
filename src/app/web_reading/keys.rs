//! Read-only page controls cannot mutate the article beneath the web view.

use super::super::{Action, App, InputKind, Mode};
use crate::reader::{cursor, motion::WordMotion};
use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

impl App {
    pub(in crate::app) fn web_mouse(
        &mut self,
        event: crossterm::event::MouseEvent,
    ) -> Result<Action> {
        use crossterm::event::{MouseButton, MouseEventKind};
        match event.kind {
            MouseEventKind::ScrollDown => self.web_scroll(true, 1),
            MouseEventKind::ScrollUp => self.web_scroll(false, 1),
            MouseEventKind::Down(MouseButton::Left) => {
                self.web.mouse_anchor =
                    self.web_pointer_byte(event.column, event.row);
                if let (Some(anchor), Some(page)) =
                    (self.web.mouse_anchor, &mut self.web.current)
                {
                    page.cursor = anchor;
                    page.selection = None;
                    page.row_goal = None;
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                let offset = self.web_pointer_byte(event.column, event.row);
                if let (Some(anchor), Some(offset), Some(page)) =
                    (self.web.mouse_anchor, offset, &mut self.web.current)
                {
                    page.selection = Some(anchor);
                    page.linewise = false;
                    page.cursor = offset;
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                self.web.mouse_anchor = None
            }
            _ => {}
        }
        Ok(Action::None)
    }

    fn web_pointer_byte(&self, column: u16, row: u16) -> Option<usize> {
        let page = self.web.current.as_ref()?;
        let area = crate::ui::web_content(self.screen, page.width);
        if !area.contains((column, row).into()) {
            return None;
        }
        let row = page.scroll + usize::from(row - area.y);
        let range = page.rendered.text_ranges.get(row)?;
        let column = usize::from(column - area.x) + page.horizontal(row);
        Some(range.start + page.columns[row].byte_at(&page.rows[row], column))
    }

    pub(in crate::app) fn web_scroll(&mut self, forward: bool, amount: usize) {
        let Some(page) = &mut self.web.current else {
            return;
        };
        let height = usize::from(self.screen.height.saturating_sub(6)).max(1);
        page.scroll = if forward {
            page.scroll
                .saturating_add(amount)
                .min(page.rendered.lines.len().saturating_sub(height))
        } else {
            page.scroll.saturating_sub(amount)
        };
        let row = page
            .cursor_row()
            .clamp(page.scroll, page.scroll + height - 1);
        if let Some(range) = page.rendered.text_ranges.get(row) {
            page.cursor = page.cursor.clamp(range.start, range.end);
        }
    }

    pub(in crate::app) fn web_key(&mut self, key: KeyEvent) -> Result<Action> {
        if key.kind == KeyEventKind::Repeat && key.code == KeyCode::Enter {
            return Ok(Action::None);
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('o') => self.web_history(false),
                KeyCode::Char('i') => self.web_history(true),
                KeyCode::Char('d') => self.web_move_rows(true, 10),
                KeyCode::Char('u') => self.web_move_rows(false, 10),
                _ => {}
            }
            return Ok(Action::None);
        }
        match key.code {
            KeyCode::Esc
                if self
                    .web
                    .current
                    .as_ref()
                    .is_some_and(|page| page.selection.is_some()) =>
            {
                if let Some(page) = &mut self.web.current {
                    page.selection = None;
                }
            }
            KeyCode::Esc | KeyCode::Char('q') => self.close_web(),
            KeyCode::Char(':') => {
                self.begin_input(InputKind::Command, String::new())
            }
            KeyCode::Enter => return self.web_follow_link(),
            KeyCode::Char('?') => self.mode = Mode::Help,
            KeyCode::Char('o') => return Ok(self.web_browser()),
            KeyCode::Char('j') | KeyCode::Down => self.web_move_rows(true, 1),
            KeyCode::Char('k') | KeyCode::Up => self.web_move_rows(false, 1),
            KeyCode::PageDown => self.web_move_rows(true, 20),
            KeyCode::PageUp => self.web_move_rows(false, 20),
            KeyCode::Char('y') => return self.web_yank(),
            code => self.web_motion(code),
        }
        Ok(Action::None)
    }

    fn web_browser(&self) -> Action {
        self.web.current.as_ref().map_or(Action::None, |page| {
            Action::Open(page.document.url.clone())
        })
    }

    pub(in crate::app) fn web_move_rows(
        &mut self,
        forward: bool,
        amount: usize,
    ) {
        let height = usize::from(self.screen.height.saturating_sub(6));
        let Some(page) = &mut self.web.current else {
            return;
        };
        let ranges = &page.rendered.text_ranges;
        let row = page.cursor_row();
        let Some(current) = ranges.get(row) else {
            return;
        };
        let text = &page.rendered.text;
        let goal = page.row_goal.unwrap_or_else(|| {
            cursor::column(
                &text[current.clone()],
                page.cursor.saturating_sub(current.start),
            )
        });
        let target = if forward {
            row.saturating_add(amount)
                .min(ranges.len().saturating_sub(1))
        } else {
            row.saturating_sub(amount)
        };
        if let Some(range) = ranges.get(target) {
            page.cursor =
                range.start + cursor::byte_at(&text[range.clone()], goal);
            page.row_goal = Some(goal);
        }
        page.reveal(height);
    }

    fn web_motion(&mut self, code: KeyCode) {
        let height = usize::from(self.screen.height.saturating_sub(6));
        let Some(page) = &mut self.web.current else {
            return;
        };
        let last =
            cursor::previous(&page.rendered.text, page.rendered.text.len());
        let text = &page.rendered.text;
        page.row_goal = None;
        let range = page
            .rendered
            .text_ranges
            .get(page.cursor_row())
            .cloned()
            .unwrap_or(0..0);
        page.cursor = match code {
            KeyCode::Char('h') | KeyCode::Left => {
                cursor::previous(text, page.cursor)
            }
            KeyCode::Char('l') | KeyCode::Right => {
                cursor::next(text, page.cursor).min(last)
            }
            KeyCode::Char('w' | 'W' | 'b' | 'B' | 'e' | 'E') => {
                word_motion(page, code, last)
            }
            KeyCode::Char('0' | 'I') | KeyCode::Home => range.start,
            KeyCode::Char('$' | 'A') | KeyCode::End => {
                cursor::previous(text, range.end).max(range.start)
            }
            KeyCode::Char('G') => last,
            KeyCode::Char('g') => 0,
            KeyCode::Char('v' | 'V') => {
                page.linewise = code == KeyCode::Char('V');
                page.selection = Some(page.cursor);
                page.cursor
            }
            _ => page.cursor,
        };
        page.reveal(height);
    }

    fn web_follow_link(&mut self) -> Result<Action> {
        let page = self.web.current.as_mut().context("Wait for the page")?;
        let address = page
            .cursor_link()
            .context("Place the cursor on a link")?
            .to_owned();
        let destination =
            url::Url::parse(&page.document.url)?.join(&address)?;
        let mut base = destination.clone();
        base.set_fragment(None);
        if base.as_str() == page.document.url
            && destination.fragment().is_some()
        {
            let fragment = destination.fragment().unwrap_or_default();
            let previous = page.source_anchor();
            page.jump_fragment(fragment)?;
            page.back.push(previous);
            if page.back.len() > 64 {
                page.back.remove(0);
            }
            page.forward.clear();
            page.reveal(usize::from(self.screen.height.saturating_sub(6)));
        } else {
            self.open_web_page(destination.as_str())?;
        }
        Ok(Action::None)
    }

    fn web_history(&mut self, forward: bool) {
        self.web.begin_navigation();
        if self.cancel_web_open() {
            return;
        }
        if self.web_local_history(forward) {
            return;
        }
        if let Some(worker) = &mut self.web.worker {
            worker.cancel();
        }
        let target = if forward {
            self.web.forward.pop()
        } else {
            self.web.back.pop_back()
        };
        let Some(target) = target else {
            if !forward {
                self.close_web();
            }
            return;
        };
        self.web.requested_width = target.width;
        if let Some(page) = self.web.current.replace(target) {
            if forward {
                self.web.back.push_back(page);
            } else {
                self.web.forward.push(page);
            }
        }
        self.mode = Mode::Web;
    }

    fn cancel_web_open(&mut self) -> bool {
        if !self.web.opening {
            return false;
        }
        self.web.opening = false;
        if let Some(worker) = &mut self.web.worker {
            worker.cancel();
        }
        if let Some(page) = &self.web.current {
            self.web.requested_width = page.width;
            self.status =
                "Pending page discarded · previous reading retained".into();
        } else {
            self.close_web();
        }
        true
    }

    fn web_local_history(&mut self, forward: bool) -> bool {
        let Some(page) = &mut self.web.current else {
            return false;
        };
        let target = if forward {
            page.forward.pop()
        } else {
            page.back.pop()
        };
        let Some(target) = target else {
            return false;
        };
        let current = page.source_anchor();
        if forward {
            page.back.push(current);
        } else {
            page.forward.push(current);
        }
        page.seek_source(target);
        page.reveal(usize::from(self.screen.height.saturating_sub(6)));
        true
    }

    fn web_yank(&self) -> Result<Action> {
        let page = self.web.current.as_ref().context("Wait for the page")?;
        let range =
            page.selected_range().context("Select text with v first")?;
        let text = &page.rendered.text[range];
        anyhow::ensure!(text.len() <= 65_536, "Selection exceeds 64 KiB");
        Ok(Action::Copy(text.to_owned()))
    }
}

fn word_motion(page: &super::WebPage, code: KeyCode, last: usize) -> usize {
    let (motion, wide) = match code {
        KeyCode::Char('w' | 'W') => {
            (WordMotion::Next, code == KeyCode::Char('W'))
        }
        KeyCode::Char('b' | 'B') => {
            (WordMotion::Previous, code == KeyCode::Char('B'))
        }
        _ => (WordMotion::End, code == KeyCode::Char('E')),
    };
    page.words.movement(page.cursor, motion, wide, 1, last)
}
