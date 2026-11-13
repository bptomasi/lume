//! Read-only document motions over cached row and word indexes.
use super::ReaderState;
use crate::app::vim::Find;
use crate::reader::{cursor, motion::WordMotion};
use crossterm::event::KeyCode;

impl ReaderState {
    pub(in crate::app) fn move_word(
        &mut self,
        motion: WordMotion,
        wide: bool,
        count: usize,
    ) {
        let target = self.projection.words.movement(
            self.cursor_offset(),
            motion,
            wide,
            count,
            cursor::previous(&self.projection.text, self.projection.text.len()),
        );
        self.viewport.pending_anchor = None;
        self.viewport.goal = None;
        self.place_cursor(target);
        self.ensure_cursor_visible();
    }

    pub(in crate::app) fn move_characters(
        &mut self,
        forward: bool,
        count: usize,
    ) {
        // Traverse cached display rows so soft wraps and blank rows remain
        // navigable without scanning the article or splitting a grapheme.
        for _ in 0..count {
            let line = self.cursor_line();
            if forward {
                let next = cursor::next(line, self.viewport.byte);
                if next < line.len() {
                    self.viewport.byte = next;
                } else if self.viewport.row + 1 < self.projection.lines.len() {
                    self.viewport.row += 1;
                    self.viewport.byte = 0;
                } else {
                    break;
                }
            } else if self.viewport.byte > 0 {
                self.viewport.byte = cursor::previous(line, self.viewport.byte);
            } else if self.viewport.row > 0 {
                self.viewport.row -= 1;
                let previous = self.cursor_line();
                self.viewport.byte = cursor::previous(previous, previous.len());
            } else {
                break;
            }
        }
        self.viewport.goal = None;
        self.ensure_cursor_visible();
    }

    pub(in crate::app) fn goto_column(&mut self, column: usize) {
        self.viewport.byte = self.row_byte_at(self.viewport.row, column);
        self.viewport.goal = Some(column);
        self.ensure_cursor_visible();
    }

    pub(in crate::app) fn control_destination(
        &mut self,
        code: KeyCode,
        count: usize,
        remember: bool,
    ) {
        match code {
            KeyCode::Left => self.move_word(WordMotion::Previous, false, count),
            KeyCode::Right => self.move_word(WordMotion::Next, false, count),
            KeyCode::Home => self.goto_reader_row(0, remember),
            KeyCode::End => self.goto_reader_row(
                self.projection.lines.len().saturating_sub(1),
                remember,
            ),
            _ => {}
        }
        self.viewport.pending_anchor = None;
    }

    pub(in crate::app) fn first_nonblank(&mut self) {
        self.viewport.byte = self
            .cursor_line()
            .char_indices()
            .find(|(_, character)| !character.is_whitespace())
            .map_or(0, |(byte, _)| byte);
        self.viewport.goal = None;
        self.ensure_cursor_visible();
    }

    pub(in crate::app) fn goto_reader_row(
        &mut self,
        row: usize,
        remember: bool,
    ) {
        if remember && row != self.viewport.row {
            self.remember_reader_jump();
        }
        self.viewport.row =
            row.min(self.projection.lines.len().saturating_sub(1));
        self.first_nonblank();
    }

    pub(in crate::app) fn viewport_row(
        &mut self,
        code: KeyCode,
        count: usize,
        remember: bool,
    ) {
        let bottom = (self.viewport.first + self.viewport.height)
            .min(self.projection.lines.len())
            .saturating_sub(1);
        let row = match code {
            KeyCode::Char('H') => {
                self.viewport.first.saturating_add(count - 1).min(bottom)
            }
            KeyCode::Char('M') => {
                self.viewport.first
                    + bottom.saturating_sub(self.viewport.first) / 2
            }
            _ => bottom.saturating_sub(count - 1).max(self.viewport.first),
        };
        self.goto_reader_row(row, remember);
    }

    pub(in crate::app) fn vim_g_key(
        &mut self,
        code: KeyCode,
        count: Option<usize>,
        remember: bool,
    ) {
        match code {
            KeyCode::Char('g') => {
                self.goto_reader_row(count.unwrap_or(1) - 1, remember)
            }
            KeyCode::Char('e' | 'E') => self.move_word(
                WordMotion::PreviousEnd,
                code == KeyCode::Char('E'),
                count.unwrap_or(1),
            ),
            KeyCode::Char('j' | 'k') => {
                self.move_reader(code == KeyCode::Char('j'), count.unwrap_or(1))
            }
            KeyCode::Char('0') => self.line_edge(false),
            KeyCode::Char('^') => self.first_nonblank(),
            KeyCode::Char('$') => self.line_edge(true),
            KeyCode::Char('_') => {
                self.line_edge(true);
                self.viewport.byte = cursor::previous(
                    self.cursor_line().trim_end(),
                    self.cursor_line().trim_end().len(),
                );
            }
            _ => {
                self.motion_status =
                    Some("Unknown reader g motion · :help vim".into())
            }
        }
        self.viewport.pending_anchor = None;
        self.ensure_cursor_visible();
    }

    pub(in crate::app) fn vim_z_key(&mut self, code: KeyCode, count: usize) {
        self.select_horizontal_block();
        match code {
            KeyCode::Char('t') => self.viewport.first = self.viewport.row,
            KeyCode::Char('z') => {
                self.viewport.first =
                    self.viewport.row.saturating_sub(self.viewport.height / 2)
            }
            KeyCode::Char('b') => {
                self.viewport.first = self
                    .viewport
                    .row
                    .saturating_sub(self.viewport.height.saturating_sub(1))
            }
            KeyCode::Char('h' | 'H') => {
                self.pan_horizontal(false, count, code == KeyCode::Char('H'))
            }
            KeyCode::Char('l' | 'L') => {
                self.pan_horizontal(true, count, code == KeyCode::Char('L'))
            }
            _ => {
                self.motion_status =
                    Some("Unknown reader z motion · :help vim".into())
            }
        }
        self.viewport.pending_anchor = None;
        self.clamp_scroll();
    }

    pub(in crate::app) fn vim_object(&mut self, code: KeyCode, around: bool) {
        let range = match code {
            KeyCode::Char('w' | 'W') => self.projection.words.object(
                &self.projection.text,
                self.cursor_offset(),
                code == KeyCode::Char('W'),
                around,
            ),
            KeyCode::Char('p') => self.paragraph_object(around),
            _ => None,
        };
        if let Some(range) = range {
            self.viewport.selection = Some(range.start);
            self.viewport.linewise = false;
            self.place_cursor(cursor::previous(
                &self.projection.text,
                range.end,
            ));
            self.ensure_cursor_visible();
        }
    }

    pub(in crate::app) fn find_character(
        &mut self,
        find: Find,
        count: usize,
        repeat: bool,
    ) {
        let line = self.cursor_line();
        let target =
            character_target(line, self.viewport.byte, find, count, repeat);
        if let Some(target) = target {
            self.viewport.byte = if find.till {
                if find.forward {
                    cursor::previous(line, target)
                } else {
                    cursor::next(line, target)
                }
            } else {
                cursor::boundary(line, target)
            };
            self.viewport.goal = None;
            self.viewport.pending_anchor = None;
            self.ensure_cursor_visible();
        } else {
            self.motion_status = Some(format!(
                "Character {:?} not found in this row",
                find.character
            ));
        }
    }

    pub(in crate::app) fn paragraph(&mut self, forward: bool, count: usize) {
        for _ in 0..count {
            self.viewport.row =
                self.paragraph_destination(self.viewport.row, forward);
        }
        self.first_nonblank();
    }

    pub(super) fn paragraph_object(
        &self,
        around: bool,
    ) -> Option<std::ops::Range<usize>> {
        let mut first = self.viewport.row;
        let mut last = first;
        while first > 0 && !self.projection.lines[first - 1].trim().is_empty() {
            first -= 1;
        }
        while last + 1 < self.projection.lines.len()
            && !self.projection.lines[last + 1].trim().is_empty()
        {
            last += 1;
        }
        if around {
            while last + 1 < self.projection.lines.len()
                && self.projection.lines[last + 1].trim().is_empty()
            {
                last += 1;
            }
        }
        Some(
            self.projection.ranges.get(first)?.start
                ..self.projection.ranges.get(last)?.end,
        )
    }

    pub(in crate::app) fn percent_motion(
        &mut self,
        count: Option<usize>,
        remember: bool,
    ) {
        if let Some(percent) = count {
            self.goto_reader_row(
                (self.projection.lines.len() * percent.min(100))
                    .div_ceil(100)
                    .saturating_sub(1),
                remember,
            );
            return;
        }
        if let Some(target) =
            matching_bracket(&self.projection.text, self.cursor_offset())
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

impl ReaderState {
    fn pan_horizontal(&mut self, right: bool, count: usize, half_page: bool) {
        let step = if half_page {
            self.viewport.width / 2
        } else {
            1
        };
        let distance = count * step;
        self.viewport.horizontal = if right {
            self.viewport.horizontal.saturating_add(distance).min(4096)
        } else {
            self.viewport.horizontal.saturating_sub(distance)
        };
    }

    fn adjacent_row(&self, row: usize, forward: bool) -> usize {
        if forward {
            row.saturating_add(1)
                .min(self.projection.lines.len().saturating_sub(1))
        } else {
            row.saturating_sub(1)
        }
    }

    fn paragraph_destination(&self, mut row: usize, forward: bool) -> usize {
        while self
            .projection
            .lines
            .get(row)
            .is_some_and(|line| line.trim().is_empty())
        {
            let next = self.adjacent_row(row, forward);
            if next == row {
                return row;
            }
            row = next;
        }
        loop {
            let next = self.adjacent_row(row, forward);
            if next == row {
                return row;
            }
            row = next;
            if self
                .projection
                .lines
                .get(row)
                .is_none_or(|line| line.trim().is_empty())
            {
                return row;
            }
        }
    }
}
