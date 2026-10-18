//! Reader motion destinations and captured character-search state.

use crossterm::event::KeyCode;
use unicode_segmentation::UnicodeSegmentation;

use crate::{
    app::App,
    reader::{cursor, motion::WordMotion},
};

use super::Find;

impl App {
    pub(super) fn move_word(
        &mut self,
        motion: WordMotion,
        wide: bool,
        count: usize,
    ) {
        let target = self.vim.words.movement(
            self.cursor_offset(),
            motion,
            wide,
            count,
            cursor::previous(&self.reader_text, self.reader_text.len()),
        );
        self.pending_anchor = None;
        self.reader_goal = None;
        self.place_cursor(target);
        self.ensure_cursor_visible();
    }

    pub(super) fn move_characters(&mut self, forward: bool, count: usize) {
        let line = self.cursor_line();
        let target = if forward {
            line.get(self.reader_column..)
                .unwrap_or_default()
                .grapheme_indices(true)
                .nth(count)
                .map_or_else(
                    || cursor::previous(line, line.len()),
                    |(byte, _)| self.reader_column + byte,
                )
        } else {
            line.get(..self.reader_column)
                .unwrap_or_default()
                .grapheme_indices(true)
                .rev()
                .nth(count - 1)
                .map_or(0, |(byte, _)| byte)
        };
        self.reader_column = target;
        self.reader_goal = None;
        self.ensure_cursor_visible();
    }

    pub(super) fn first_nonblank(&mut self) {
        self.reader_column = self
            .cursor_line()
            .char_indices()
            .find(|(_, character)| !character.is_whitespace())
            .map_or(0, |(byte, _)| byte);
        self.reader_goal = None;
        self.ensure_cursor_visible();
    }

    pub(super) fn goto_reader_row(&mut self, row: usize) {
        if row != self.reader_cursor {
            self.remember_reader_jump();
        }
        self.reader_cursor = row.min(self.reader_lines.len().saturating_sub(1));
        self.first_nonblank();
    }

    pub(super) fn viewport_row(&mut self, code: KeyCode, count: usize) {
        let bottom = (self.scroll + self.reader_height)
            .min(self.reader_lines.len())
            .saturating_sub(1);
        let row = match code {
            KeyCode::Char('H') => {
                self.scroll.saturating_add(count - 1).min(bottom)
            }
            KeyCode::Char('M') => {
                self.scroll + bottom.saturating_sub(self.scroll) / 2
            }
            _ => bottom.saturating_sub(count - 1).max(self.scroll),
        };
        self.goto_reader_row(row);
    }

    pub(super) fn vim_g_key(&mut self, code: KeyCode, count: Option<usize>) {
        match code {
            KeyCode::Char('g') => self.goto_reader_row(count.unwrap_or(1) - 1),
            KeyCode::Char('e' | 'E') => self.move_word(
                WordMotion::PreviousEnd,
                code == KeyCode::Char('E'),
                count.unwrap_or(1),
            ),
            KeyCode::Char('j' | 'k') => {
                self.move_reader(code == KeyCode::Char('j'), count.unwrap_or(1))
            }
            KeyCode::Char('0') => self.line_edge(false),
            KeyCode::Char('$') => self.line_edge(true),
            KeyCode::Char('_') => {
                self.line_edge(true);
                self.reader_column = cursor::previous(
                    self.cursor_line().trim_end(),
                    self.cursor_line().trim_end().len(),
                );
            }
            _ => self.status = "Unknown reader g motion · :help vim".into(),
        }
        self.pending_anchor = None;
        self.ensure_cursor_visible();
    }

    pub(super) fn vim_z_key(&mut self, code: KeyCode, count: usize) {
        match code {
            KeyCode::Char('t') => self.scroll = self.reader_cursor,
            KeyCode::Char('z') => {
                self.scroll =
                    self.reader_cursor.saturating_sub(self.reader_height / 2)
            }
            KeyCode::Char('b') => {
                self.scroll = self
                    .reader_cursor
                    .saturating_sub(self.reader_height.saturating_sub(1))
            }
            KeyCode::Char('h' | 'H') => {
                self.horizontal = self.horizontal.saturating_sub(
                    count
                        * if code == KeyCode::Char('H') {
                            self.reader_width / 2
                        } else {
                            1
                        },
                )
            }
            KeyCode::Char('l' | 'L') => {
                self.horizontal = self
                    .horizontal
                    .saturating_add(
                        count
                            * if code == KeyCode::Char('L') {
                                self.reader_width / 2
                            } else {
                                1
                            },
                    )
                    .min(4096)
            }
            _ => self.status = "Unknown reader z motion · :help vim".into(),
        }
        self.pending_anchor = None;
        self.clamp_scroll();
    }

    pub(super) fn vim_object(&mut self, code: KeyCode, around: bool) {
        let range = match code {
            KeyCode::Char('w' | 'W') => self.vim.words.object(
                &self.reader_text,
                self.cursor_offset(),
                code == KeyCode::Char('W'),
                around,
            ),
            KeyCode::Char('p') => self.paragraph_object(around),
            _ => None,
        };
        if let Some(range) = range {
            self.selection = Some(range.start);
            self.linewise = false;
            self.place_cursor(cursor::previous(&self.reader_text, range.end));
            self.ensure_cursor_visible();
        }
    }

    pub(super) fn find_character(
        &mut self,
        find: Find,
        count: usize,
        repeat: bool,
    ) {
        let line = self.cursor_line();
        let target =
            character_target(line, self.reader_column, find, count, repeat);
        if let Some(target) = target {
            self.reader_column = if find.till {
                if find.forward {
                    cursor::previous(line, target)
                } else {
                    cursor::next(line, target)
                }
            } else {
                cursor::boundary(line, target)
            };
            self.reader_goal = None;
            self.pending_anchor = None;
            self.ensure_cursor_visible();
        } else {
            self.status =
                format!("Character {:?} not found in this row", find.character);
        }
    }

    pub(super) fn paragraph(&mut self, forward: bool, count: usize) {
        for _ in 0..count {
            let mut row = self.reader_cursor;
            while self
                .reader_lines
                .get(row)
                .is_some_and(|line| line.trim().is_empty())
            {
                let next = if forward {
                    row.saturating_add(1)
                        .min(self.reader_lines.len().saturating_sub(1))
                } else {
                    row.saturating_sub(1)
                };
                if next == row {
                    break;
                }
                row = next;
            }
            loop {
                let next = if forward {
                    row.saturating_add(1)
                        .min(self.reader_lines.len().saturating_sub(1))
                } else {
                    row.saturating_sub(1)
                };
                if next == row {
                    break;
                }
                row = next;
                if self
                    .reader_lines
                    .get(row)
                    .is_none_or(|line| line.trim().is_empty())
                {
                    break;
                }
            }
            self.reader_cursor = row;
        }
        self.first_nonblank();
    }

    fn paragraph_object(&self, around: bool) -> Option<std::ops::Range<usize>> {
        let mut first = self.reader_cursor;
        let mut last = first;
        while first > 0 && !self.reader_lines[first - 1].trim().is_empty() {
            first -= 1;
        }
        while last + 1 < self.reader_lines.len()
            && !self.reader_lines[last + 1].trim().is_empty()
        {
            last += 1;
        }
        if around {
            while last + 1 < self.reader_lines.len()
                && self.reader_lines[last + 1].trim().is_empty()
            {
                last += 1;
            }
        }
        Some(
            self.reader_ranges.get(first)?.start
                ..self.reader_ranges.get(last)?.end,
        )
    }

    pub(super) fn percent_motion(&mut self, count: Option<usize>) {
        if let Some(percent) = count {
            self.goto_reader_row(
                (self.reader_lines.len() * percent.min(100))
                    .div_ceil(100)
                    .saturating_sub(1),
            );
            return;
        }
        if let Some(target) =
            matching_bracket(&self.reader_text, self.cursor_offset())
        {
            self.place_cursor(target);
            self.ensure_cursor_visible();
        }
    }
}

fn matching_bracket(text: &str, offset: usize) -> Option<usize> {
    let (local, first) = text
        .get(offset..)?
        .chars()
        .take_while(|character| *character != '\n')
        .enumerate()
        .find(|(_, character)| "()[]{}<>".contains(*character))?;
    let start = text.get(offset..)?.char_indices().nth(local)?.0 + offset;
    let pair = match first {
        '(' | ')' => ('(', ')'),
        '[' | ']' => ('[', ']'),
        '{' | '}' => ('{', '}'),
        _ => ('<', '>'),
    };
    let mut depth = 0;
    let forward = first == pair.0;
    let mut characters: Box<dyn Iterator<Item = (usize, char)> + '_> =
        if forward {
            Box::new(text.char_indices().filter(|(byte, _)| *byte >= start))
        } else {
            Box::new(
                text.char_indices().rev().filter(|(byte, _)| *byte <= start),
            )
        };
    characters.find_map(|(byte, character)| {
        if character == first {
            depth += 1;
        } else if character == if forward { pair.1 } else { pair.0 } {
            depth -= 1;
            if depth == 0 {
                return Some(byte);
            }
        }
        None
    })
}

fn character_target(
    line: &str,
    current: usize,
    find: Find,
    count: usize,
    repeat: bool,
) -> Option<usize> {
    let offset = if repeat && find.till {
        if find.forward {
            cursor::next(line, current)
        } else {
            cursor::previous(line, current)
        }
    } else {
        current
    };
    if find.forward {
        line.char_indices()
            .filter(|(byte, character)| {
                *byte > offset && *character == find.character
            })
            .nth(count - 1)
            .map(|(byte, _)| byte)
    } else {
        line.char_indices()
            .rev()
            .filter(|(byte, character)| {
                *byte < offset && *character == find.character
            })
            .nth(count - 1)
            .map(|(byte, _)| byte)
    }
}
