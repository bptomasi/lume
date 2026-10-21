//! One lazy row projection keeps visible rendering and pointer hits consistent.
//!
//! Section transitions add a blank row and heading. Offscreen entries contribute
//! only row identities; text allocation and formatting are limited to the viewport.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::Line,
    widgets::Paragraph,
};

use crate::app::App;

use super::theme::{ACCENT, FAINT, MUTED, RAISED};

#[derive(Clone, Copy)]
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
    let first = first(app, area.height);
    let lines = rows(app)
        .skip(first)
        .take(usize::from(area.height))
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
    match rows(app).nth(first(app, height) + row)? {
        Row::Item(index) => Some(index),
        Row::Blank | Row::Heading(_) => None,
    }
}

fn first(app: &App, height: u16) -> usize {
    rows(app)
        .position(
            |row| matches!(row, Row::Item(index) if index == app.nav_selected),
        )
        .unwrap_or(0)
        .saturating_sub(usize::from(height) / 2)
}

fn rows(app: &App) -> impl Iterator<Item = Row> + '_ {
    let mut group = "";
    app.navigation_indices().flat_map(move |index| {
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
    let marker = if selected { "▎" } else { " " };
    let title =
        crate::content::label(&item.title, width.saturating_sub(8) as usize);
    let title_width = usize::from(width.saturating_sub(6));
    let text = format!("{marker} {title:<title_width$}{:>3} ", item.count);
    let style = if selected {
        Style::new()
            .fg(ACCENT)
            .bg(RAISED)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(MUTED)
    };
    Line::styled(text, style)
}
