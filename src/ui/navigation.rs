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

use crate::presentation::navigation::{NavItem, NavigationView, Row, window};

use super::theme::{ACCENT, FAINT, MUTED, RAISED};

pub(super) fn draw(frame: &mut Frame<'_>, app: NavigationView<'_>, area: Rect) {
    if app.indices().next().is_none() {
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

fn item_line(
    app: NavigationView<'_>,
    index: usize,
    width: u16,
) -> Line<'static> {
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

fn section_text(item: &NavItem, width: u16) -> String {
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

fn item_text(item: &NavItem, selected: bool, width: u16) -> String {
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
