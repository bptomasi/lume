//! One bounded row projection keeps visible rendering and pointer hits consistent.
//!
//! Keep the viewport stable while selection remains visible; scroll only enough
//! to reveal an offscreen destination. Draw and hit testing share the same rows.

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
    let (first, skip) = window_start(app, height);
    rows(app, first).skip(skip).take(usize::from(height))
}

pub(crate) fn origin(app: &App, screen: Rect) -> Option<(usize, usize)> {
    let area =
        super::workspace_areas(screen, app.focus, &app.workspace).library?;
    let inner = super::pane_search::content(
        app,
        crate::model::Pane::Library,
        area.inner(ratatui::layout::Margin::new(1, 1)),
    );
    (inner.height > 0).then(|| window_start(app, inner.height))
}

fn window_start(app: &App, height: u16) -> (usize, usize) {
    let selected = app.navigation_position(app.nav_selected).unwrap_or(0);
    if selected < app.navigation_first {
        return lookbehind(app, selected, 0);
    }
    let first = app.navigation_first.min(selected);
    let skip = if first == app.navigation_first {
        app.navigation_skip.min(heading_rows(app, first))
    } else {
        0
    };
    if rows(app, first)
        .skip(skip)
        .take(usize::from(height))
        .any(|row| row == Row::Item(app.nav_selected))
    {
        return (first, skip);
    }
    lookbehind(app, selected, usize::from(height).saturating_sub(1))
}

fn lookbehind(
    app: &App,
    mut first: usize,
    mut remaining: usize,
) -> (usize, usize) {
    let mut skip = heading_rows(app, first);
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
    (first, skip)
}

fn heading_rows(app: &App, position: usize) -> usize {
    let Some(index) = app.navigation_indices().nth(position) else {
        return 0;
    };
    let previous = position
        .checked_sub(1)
        .and_then(|position| app.navigation_indices().nth(position))
        .map_or("", |index| app.navigation[index].group);
    if app.navigation[index].group == previous {
        return 0;
    }
    if matches!(app.navigation[index].view, crate::model::View::Group(_)) {
        1
    } else {
        2
    }
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
            (changed_group
                && !matches!(item.view, crate::model::View::Group(_)))
            .then_some(Row::Heading(group)),
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
    } else if matches!(item.view, crate::model::View::Group(_)) {
        Style::new().fg(FAINT)
    } else {
        Style::new().fg(MUTED)
    };
    let text = if matches!(item.view, crate::model::View::Group(_)) {
        section_text(item, width)
    } else {
        item_text(item, selected, width)
    };
    Line::styled(text, style)
}

fn section_text(item: &crate::app::NavItem, width: u16) -> String {
    let label = &item.title;
    let count = item.count.to_string();
    let width = usize::from(width);
    if label.width() + count.len() < width {
        let padding = width - label.width();
        format!("{label}{count:>padding$}")
    } else {
        label.clone()
    }
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
