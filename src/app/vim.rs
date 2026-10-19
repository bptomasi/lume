//! Read-only Vim motions are reader-local; application actions keep ex commands.
//!
//! Counts and multi-key prefixes are transient input, never persisted. Motions
//! use cached readable-text boundaries and never execute SQL or parse Markdown.

mod motions;

use anyhow::Result;
use crossterm::event::KeyCode;

use crate::reader::motion::{WordIndex, WordMotion};

use super::{App, Focus};

#[derive(Clone, Copy)]
pub(super) struct Find {
    character: char,
    forward: bool,
    till: bool,
}

#[derive(Clone, Copy)]
enum Prefix {
    G(Option<usize>),
    Z(usize),
    Find {
        forward: bool,
        till: bool,
        count: usize,
    },
    Object(bool),
}

#[derive(Default)]
pub(super) struct Vim {
    pub(super) words: WordIndex,
    count: usize,
    prefix: Option<Prefix>,
    last_find: Option<Find>,
}

impl Vim {
    pub(super) fn prefix_pending(&self) -> bool {
        self.prefix.is_some()
    }

    pub(super) fn clear_input(&mut self) {
        self.count = 0;
        self.prefix = None;
    }
}

impl App {
    pub(super) fn vim_pending(&self) -> bool {
        self.panel_prefix
            || self.vim.count > 0
            || self.vim.prefix.is_some()
            || self.note_marks.delete.is_some()
    }

    pub(super) fn reader_vim_key(&mut self, code: KeyCode) -> Result<bool> {
        if self.focus != Focus::Reader {
            return Ok(false);
        }
        if code == KeyCode::Esc && self.vim_pending() {
            self.vim.clear_input();
            return Ok(true);
        }
        if let Some(prefix) = self.vim.prefix.take() {
            self.finish_vim_prefix(prefix, code);
            return Ok(true);
        }
        if self.vim_digit(code) {
            return Ok(true);
        }
        let explicit = std::mem::take(&mut self.vim.count);
        if explicit > 0 {
            self.status = "Reader motion · :help vim".into();
        }
        let count = explicit.max(1);
        if let Some((motion, wide)) = word_command(code) {
            self.move_word(motion, wide, count);
            return Ok(true);
        }
        if self.vim_line_key(code, count) || self.vim_prefix_key(code, explicit)
        {
            return Ok(true);
        }
        self.vim_extra_key(code, count, explicit > 0)
    }

    fn vim_digit(&mut self, code: KeyCode) -> bool {
        let KeyCode::Char(character @ '0'..='9') = code else {
            return false;
        };
        if character == '0' && self.vim.count == 0 {
            return false;
        }
        let digit = character.to_digit(10).unwrap_or(0) as usize;
        self.vim.count = (self.vim.count * 10 + digit).min(9999);
        self.status = format!("Reader count: {}", self.vim.count);
        true
    }

    fn vim_prefix_key(&mut self, code: KeyCode, explicit: usize) -> bool {
        let count = explicit.max(1);
        self.vim.prefix = match code {
            KeyCode::Char('g') => {
                Some(Prefix::G((explicit > 0).then_some(explicit)))
            }
            KeyCode::Char('z') => Some(Prefix::Z(count)),
            KeyCode::Char('f' | 'F' | 't' | 'T') => Some(Prefix::Find {
                forward: matches!(code, KeyCode::Char('f' | 't')),
                till: matches!(code, KeyCode::Char('t' | 'T')),
                count,
            }),
            KeyCode::Char('i' | 'a') if self.selection.is_some() => {
                Some(Prefix::Object(code == KeyCode::Char('a')))
            }
            _ => return false,
        };
        true
    }

    fn finish_vim_prefix(&mut self, prefix: Prefix, code: KeyCode) {
        match prefix {
            Prefix::G(count) => self.vim_g_key(code, count),
            Prefix::Z(count) => self.vim_z_key(code, count),
            Prefix::Object(around) => self.vim_object(code, around),
            Prefix::Find {
                forward,
                till,
                count,
            } => {
                if let KeyCode::Char(character) = code {
                    let find = Find {
                        character,
                        forward,
                        till,
                    };
                    self.vim.last_find = Some(find);
                    self.find_character(find, count, false);
                }
            }
        }
    }

    fn vim_line_key(&mut self, code: KeyCode, count: usize) -> bool {
        match code {
            KeyCode::Char('h') | KeyCode::Left | KeyCode::Backspace => {
                self.move_characters(false, count)
            }
            KeyCode::Char('l') | KeyCode::Right => {
                self.move_characters(true, count)
            }
            KeyCode::Char('j') | KeyCode::Down => self.move_reader(true, count),
            KeyCode::Char('k') | KeyCode::Up => self.move_reader(false, count),
            KeyCode::Char('0') | KeyCode::Home => self.line_edge(false),
            KeyCode::Char('$' | 'A') | KeyCode::End => {
                self.move_reader(true, count - 1);
                self.line_edge(true);
            }
            KeyCode::Char('^' | 'I') => self.first_nonblank(),
            KeyCode::Char('|') => self.goto_column(count - 1),
            KeyCode::Char('_') => {
                self.move_reader(true, count - 1);
                self.first_nonblank();
            }
            KeyCode::Char('+') | KeyCode::Char('-') => {
                self.move_reader(code == KeyCode::Char('+'), count);
                self.first_nonblank();
            }
            _ => return false,
        }
        self.pending_anchor = None;
        true
    }

    fn vim_extra_key(
        &mut self,
        code: KeyCode,
        count: usize,
        explicit: bool,
    ) -> Result<bool> {
        match code {
            KeyCode::Char('G') => self.goto_reader_row(if explicit {
                count - 1
            } else {
                self.reader_lines.len().saturating_sub(1)
            }),
            KeyCode::Char('H' | 'M' | 'L') => self.viewport_row(code, count),
            KeyCode::Char('{' | '}') => {
                self.paragraph(code == KeyCode::Char('}'), count)
            }
            KeyCode::Char('%') => {
                self.percent_motion(explicit.then_some(count))
            }
            KeyCode::Char(';') | KeyCode::Char(',') => {
                if let Some(mut find) = self.vim.last_find {
                    if code == KeyCode::Char(',') {
                        find.forward = !find.forward;
                    }
                    self.find_character(find, count, true);
                }
            }
            KeyCode::Char('n' | 'N')
                if self.selection.is_none()
                    && !self.reader_tools.needle.is_empty() =>
            {
                for _ in 0..count {
                    self.next_reader_match(code == KeyCode::Char('n'))?;
                }
            }
            _ => return Ok(false),
        }
        self.pending_anchor = None;
        Ok(true)
    }
}

fn word_command(code: KeyCode) -> Option<(WordMotion, bool)> {
    let result = match code {
        KeyCode::Char('w') => (WordMotion::Next, false),
        KeyCode::Char('W') => (WordMotion::Next, true),
        KeyCode::Char('b') => (WordMotion::Previous, false),
        KeyCode::Char('B') => (WordMotion::Previous, true),
        KeyCode::Char('e') => (WordMotion::End, false),
        KeyCode::Char('E') => (WordMotion::End, true),
        _ => return None,
    };
    Some(result)
}

impl App {
    pub(super) fn reader_vim_control(&mut self, code: KeyCode) -> Result<bool> {
        if self.focus != Focus::Reader {
            return Ok(false);
        }
        let count = self.vim.count.max(1);
        if matches!(
            code,
            KeyCode::Left | KeyCode::Right | KeyCode::Home | KeyCode::End
        ) {
            self.vim.clear_input();
            self.control_destination(code, count);
            return Ok(true);
        }
        let (forward, distance) = match code {
            KeyCode::Char('f' | 'b') => {
                (code == KeyCode::Char('f'), count * self.reader_height)
            }
            KeyCode::Char('d' | 'u') => (
                code == KeyCode::Char('d'),
                count * (self.reader_height / 2).max(1),
            ),
            KeyCode::Char('e' | 'y') => {
                self.scroll_reader(code == KeyCode::Char('e'), count);
                self.vim.clear_input();
                return Ok(true);
            }
            KeyCode::Char('o' | 'i') => {
                self.vim.clear_input();
                if code == KeyCode::Char('o') && self.return_to_notebook()? {
                    return Ok(true);
                }
                for _ in 0..count {
                    self.jump_history(code == KeyCode::Char('i'))?;
                }
                return Ok(true);
            }
            _ => return Ok(false),
        };
        self.vim.clear_input();
        self.move_reader(forward, distance);
        Ok(true)
    }

    pub(super) fn scroll_reader(&mut self, forward: bool, count: usize) {
        self.pending_anchor = None;
        self.scroll = if forward {
            self.scroll.saturating_add(count)
        } else {
            self.scroll.saturating_sub(count)
        };
        self.scroll = self
            .scroll
            .min(self.reader_lines.len().saturating_sub(self.reader_height));
        self.reader_cursor = self.reader_cursor.clamp(
            self.scroll,
            (self.scroll + self.reader_height)
                .min(self.reader_lines.len())
                .saturating_sub(1)
                .max(self.scroll),
        );
        self.clamp_cursor();
    }
}
