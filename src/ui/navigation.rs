//! One bounded row projection keeps visible rendering and pointer hits consistent.
//!
//! Walk backward only far enough to center the selection, then stream visible
//! rows. Indexed destinations avoid scanning the preceding catalog on each frame.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::Line,
    widgets::Paragraph,
};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::app::App;

use super::theme::{ACCENT, FAINT, MUTED, RAISED};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Row {
    Blank,
    Heading(&'static str),
    Item(usize),
}

pub(super) fn draw(frame: &mut Frame<'_>, app: &App, area: Rect) {
    if app.navigation_indices().next().is_none() {
        frame.render_widget(Paragraph::new("No matching destinations"), area);
        return;
    }
    let lines = window(app, area.height)
        .map(|row| match row {
            Row::Blank => Line::raw(""),
            Row::Heading(group) => {
                Line::styled(format!("  {group}"), Style::new().fg(FAINT))
            }
            Row::Item(index) => item_line(app, index, area.width),
        })
        .collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(lines), area);
}

pub(super) fn at(app: &App, height: u16, row: usize) -> Option<usize> {
    app.navigation.get(app.nav_selected)?;
    match window(app, height).nth(row)? {
        Row::Item(index) => Some(index),
        Row::Blank | Row::Heading(_) => None,
    }
}

fn window(app: &App, height: u16) -> impl Iterator<Item = Row> + '_ {
    // An entry owns its optional blank/heading rows before the selectable item.
    // Count only the lookbehind needed to center that item in display rows.
    let mut first = app.navigation_position(app.nav_selected).unwrap_or(0);
    let mut skip = heading_rows(app, first);
    let mut remaining = usize::from(height) / 2;
    loop {
        if remaining <= skip {
            skip -= remaining;
            break;
        }
        remaining -= skip;
        if first == 0 {
            skip = 0;
            break;
        }
        first -= 1;
        skip = heading_rows(app, first) + 1;
    }
    rows(app, first).skip(skip).take(usize::from(height))
}

fn heading_rows(app: &App, position: usize) -> usize {
    let Some(index) = app.navigation_indices().nth(position) else {
        return 0;
    };
    let previous = position
        .checked_sub(1)
        .and_then(|position| app.navigation_indices().nth(position))
        .map_or("", |index| app.navigation[index].group);
    usize::from(app.navigation[index].group != previous) * 2
}

fn rows(app: &App, first: usize) -> impl Iterator<Item = Row> + '_ {
    let mut group = first
        .checked_sub(1)
        .and_then(|position| app.navigation_indices().nth(position))
        .map_or("", |index| app.navigation[index].group);
    app.navigation_indices().skip(first).flat_map(move |index| {
        let item = &app.navigation[index];
        let changed_group = item.group != group;
        group = item.group;
        [
            changed_group.then_some(Row::Blank),
            changed_group.then_some(Row::Heading(group)),
            Some(Row::Item(index)),
        ]
        .into_iter()
        .flatten()
    })
}

fn item_line(app: &App, index: usize, width: u16) -> Line<'static> {
    let item = &app.navigation[index];
    let selected = index == app.nav_selected;
    let style = if selected {
        Style::new()
            .fg(ACCENT)
            .bg(RAISED)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(MUTED)
    };
    Line::styled(item_text(item, selected, width), style)
}

fn item_text(item: &crate::app::NavItem, selected: bool, width: u16) -> String {
    let marker = if selected { "▎" } else { " " };
    let digits = item.count.checked_ilog10().map_or(1, |value| value + 1);
    let title_width =
        usize::from(width).saturating_sub(digits.max(3) as usize + 3);
    let budget = title_width.saturating_sub(2);
    let safe = crate::content::label(&item.title, usize::from(width) * 4);
    let mut cells = 0;
    let mut end = 0;
    for grapheme in safe.graphemes(true) {
        let next = grapheme.width();
        if cells + next > budget {
            break;
        }
        cells += next;
        end += grapheme.len();
    }
    let title = &safe[..end];
    let padding = title_width.saturating_sub(cells);
    format!("{marker} {title}{:padding$}{:>3} ", "", item.count)
}

#[cfg(test)]
mod tests;
