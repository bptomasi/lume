//! Global identity, pane controls and the modal status line.

use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::{
    app::{App, Focus, Mode},
    content,
};

use super::theme::*;

pub(super) fn header(frame: &mut Frame<'_>, app: &App) {
    let area = frame.area();
    let title = Line::from(vec![
        Span::styled("  ◉ ", Style::new().fg(AMBER)),
        Span::styled(
            "lume",
            Style::new().fg(TEXT).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "  /  a quiet place for curious minds",
            Style::new().fg(MUTED),
        ),
    ]);
    frame.render_widget(Paragraph::new(title), Rect::new(0, 0, area.width, 1));
    let state = if app.demo {
        "DEMO · LOCAL"
    } else {
        "LOCAL LIBRARY"
    };
    let right = format!("● {state}  ");
    if area.width >= 90 {
        frame.render_widget(
            Paragraph::new(right)
                .alignment(Alignment::Right)
                .style(Style::new().fg(ACCENT)),
            Rect::new(area.width.saturating_sub(28), 0, 28, 1),
        );
    }
}

pub(super) fn footer(frame: &mut Frame<'_>, app: &App) {
    let area = frame.area();
    if area.height < 2 {
        return;
    }
    let status = status_line(app);
    frame.render_widget(
        Paragraph::new(status),
        Rect::new(0, area.height - 2, area.width, 1),
    );
    frame.render_widget(
        Paragraph::new(hints(app)).style(Style::new().fg(FAINT)),
        Rect::new(0, area.height - 1, area.width, 1),
    );
}

fn hints(app: &App) -> &'static str {
    if app.layout_prefix_pending() {
        return "  Ctrl-w · h/j/k/l focus · H/J/K/L move · s/v split · u/r history · Esc cancel";
    }
    if app.focus == Focus::Annotations {
        return "  j/k notes · Enter jump · e edit · n new · Ctrl-w c collapse";
    }
    if app.focus == Focus::Reader {
        if app.selection.is_some() {
            return "  y copy  ·  s highlight  ·  n note  ·  Esc cancel";
        }
        return concat!(
            "  w/W/e word  ·  / find  ·  n/N match  ·  :note  ·  ",
            "Z fullscreen  ·  ? help"
        );
    }
    concat!(
        "  space actions  ·  / search  ·  f star  ·  s queue  ·  ",
        "n note  ·  Z fullscreen  ·  ? help"
    )
}

pub(super) fn pane_controls(
    frame: &mut Frame<'_>,
    app: &App,
    areas: &super::Areas,
) {
    for (pane, area) in areas.ordered() {
        if let Some(grip) = super::pane_move_rect(area) {
            let moving = app.panel_drag().is_some_and(|drag| drag.pane == pane);
            let style = if moving {
                Style::new().fg(BG).bg(AMBER)
            } else {
                Style::new().fg(AMBER).bg(PANEL)
            };
            frame.render_widget(Paragraph::new("[≡]").style(style), grip);
        }
        if let Some(button) = super::pane_close_rect(area) {
            frame.render_widget(
                Paragraph::new("[−]").style(Style::new().fg(AMBER).bg(PANEL)),
                button,
            );
        }
    }
    if app.workspace.fullscreen {
        return;
    }
    restore_buttons(frame, app);
    if areas.ordered().is_empty() {
        collapsed_workspace(frame);
    }
}

pub(super) fn draw_panel_move(frame: &mut Frame<'_>, app: &App) {
    let Some(drag) = app.panel_drag() else {
        return;
    };
    let areas = super::workspace_areas(frame.area(), app.focus, &app.workspace);
    for (pane, area) in areas.ordered() {
        // Only decorate destinations. The source is captured intact, with
        // its original rounded corners, title and controls.
        if drag.target != Some(pane) {
            continue;
        }
        let area = if drag.edge.is_some() {
            area
        } else {
            app.pane_insertion_slot().unwrap_or(area)
        };
        let marker = match drag.edge {
            Some(crate::model::Dock::Above) => {
                Rect::new(area.x, area.y, area.width, 1)
            }
            Some(crate::model::Dock::Below) => Rect::new(
                area.x,
                area.bottom().saturating_sub(1),
                area.width,
                1,
            ),
            _ => Rect::new(area.x, area.y, 1, area.height),
        };
        let border = if marker.height == 1 {
            Borders::TOP
        } else {
            Borders::LEFT
        };
        frame.render_widget(
            Block::new()
                .borders(border)
                .border_style(Style::new().fg(BLUE)),
            marker,
        );
    }
}

fn restore_buttons(frame: &mut Frame<'_>, app: &App) {
    for (pane, button) in super::pane_buttons(frame.area(), &app.workspace) {
        let visible = app.workspace.pane_visible(pane);
        let name = if button.width < 13 {
            if pane == Focus::Annotations {
                "N"
            } else {
                &pane.title()[..1]
            }
        } else {
            pane.title()
        };
        let text = format!(" [{} {name}]", if visible { "−" } else { "+" });
        let style = if app
            .panel_drag()
            .is_some_and(|drag| drag.target == Some(pane))
        {
            Style::new().fg(BG).bg(BLUE)
        } else {
            Style::new().fg(if visible { MUTED } else { AMBER })
        };
        frame.render_widget(Paragraph::new(text).style(style), button);
    }
}

fn collapsed_workspace(frame: &mut Frame<'_>) {
    frame.render_widget(
        Paragraph::new(concat!(
            "All panes collapsed.\n\nClick a + button above, or use ",
            ":panel-focus reader.\nCtrl-w a opens your article annotations."
        ))
        .wrap(ratatui::widgets::Wrap { trim: false })
        .style(Style::new().fg(MUTED)),
        Rect::new(
            2,
            5,
            frame.area().width.saturating_sub(4),
            frame.area().height.saturating_sub(8),
        ),
    );
}

fn status_line(app: &App) -> Line<'static> {
    let (mode, color) = match &app.mode {
        Mode::Normal if app.selection.is_some() => (" VISUAL ", AMBER),
        Mode::Normal => (" NORMAL ", ACCENT),
        Mode::Help => (" HELP ", BLUE),
        Mode::Input(_) | Mode::Smart(_) => (" INSERT ", AMBER),
        Mode::Picker(_) => (" SELECT ", BLUE),
        Mode::Leader => (" SPACE ", AMBER),
        Mode::Document(_) => (" INSPECT ", AMBER),
        Mode::Settings(_) => (" PREVIEW ", AMBER),
    };
    let activity = if app.session_failed() {
        "  SAVE ERROR"
    } else if app.session_pending() {
        "  saving…"
    } else if app.refreshing {
        "  ↻ collecting feeds"
    } else {
        ""
    };
    Line::from(vec![
        Span::styled(
            mode,
            Style::new().fg(BG).bg(color).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {:?} ", app.focus),
            Style::new().fg(TEXT).bg(RAISED),
        ),
        Span::styled(activity, Style::new().fg(AMBER)),
        Span::styled(
            format!("  {}", content::label(&app.status, 300)),
            Style::new().fg(MUTED),
        ),
    ])
}
