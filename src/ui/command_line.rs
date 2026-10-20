//! Ex-command suggestions render from the fixed registry without I/O.

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::Line,
    widgets::Paragraph,
};

use crate::app::{App, CommandSpec, Input, command_suggestions};

use super::{overlays::modal, theme::*};

pub(super) fn draw(frame: &mut Frame<'_>, app: &App, input: &Input) {
    let suggestions = command_suggestions(&input.text);
    let title = format!(": Commands · {} matches", suggestions.len());
    let inner = modal(frame, &title, super::popup::Popup::Command);
    let areas = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(1),
        Constraint::Length(2),
    ])
    .split(inner);
    frame.render_widget(&input.editor, areas[0]);
    choices(frame, input, &suggestions, areas[1]);
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled(
                crate::content::label(&app.status, 160),
                Style::new().fg(MUTED),
            ),
            Line::styled(
                "↑↓ select · Tab complete · Enter run · Esc cancel",
                Style::new().fg(AMBER),
            ),
        ]),
        areas[2],
    );
}

fn choices(
    frame: &mut Frame<'_>,
    input: &Input,
    suggestions: &[&CommandSpec],
    area: Rect,
) {
    if suggestions.is_empty() {
        frame.render_widget(
            Paragraph::new("No commands match · Ctrl-u clears the draft")
                .style(Style::new().fg(MUTED)),
            area,
        );
        return;
    }
    let capacity = usize::from(area.height / 2).max(1);
    let first = input.completion.saturating_sub(capacity / 2);
    let lines = suggestions
        .iter()
        .enumerate()
        .skip(first)
        .take(capacity)
        .flat_map(|(index, command)| {
            let selected = index == input.completion;
            let style = if selected {
                Style::new().fg(AMBER).bg(RAISED)
            } else {
                Style::new().fg(TEXT)
            };
            let marker = if selected { "›" } else { " " };
            [
                Line::styled(format!("{marker} :{}", command.usage), style),
                Line::styled(
                    format!("    {}", command.description),
                    Style::new().fg(MUTED),
                ),
            ]
        })
        .collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(lines), area);
}
