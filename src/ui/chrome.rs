//! Global identity, pane controls and the modal status line.

use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::{
    app::{App, EditMode, Focus, InputKind, Mode},
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
    if let Mode::Input(input) = &app.mode
        && matches!(
            input.kind,
            InputKind::Comment { .. }
                | InputKind::Highlight { .. }
                | InputKind::StudyCard { .. }
        )
    {
        return super::overlays::editor_hint(input, true);
    }
    if app.layout_prefix_pending() {
        return "  Ctrl-w · h/j/k/l focus · H/J/K/L move · s/v split · u/r history · Esc cancel";
    }
    if app.notebook.is_some() {
        return "  / notes · e edit · Enter source · t tags · T type · p pin · r resolve · v mark · s synthesize · dd / u · Esc return";
    }
    if app.focus == Focus::Annotations {
        return concat!(
            "  j/k notes · Enter jump · e edit · dd delete · ",
            "Ctrl-w c collapse · Reader n creates"
        );
    }
    if app.focus == Focus::Reader {
        return reader_hints(app);
    }
    concat!(
        "  space actions  ·  / search  ·  f star  ·  s queue  ·  ",
        "Space / notes · Z fullscreen · ? help"
    )
}

fn reader_hints(app: &App) -> &'static str {
    if app.selection.is_some() {
        return "  y copy  ·  s highlight  ·  n note  ·  Esc cancel";
    }
    if app.cursor_note().is_some() {
        return "  Enter edit · dd delete · u undo · :annotations · Tab panels";
    }
    if app.reader_tools.needle.is_empty() {
        return "  w/W/e word · / find · n note · :annotate · Ctrl-w layout";
    }
    concat!(
        "  w/W/e word  ·  / find  ·  n/N match  ·  :note  ·  ",
        "Z fullscreen  ·  ? help"
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
        let caption = if app.notebook.is_some() {
            match pane {
                Focus::Library => "Lists",
                Focus::Articles => "Notes",
                Focus::Reader => "Source",
                Focus::Annotations => "Links",
            }
        } else {
            pane.title()
        };
        let name = if button.width < 13 {
            if pane == Focus::Annotations {
                "N"
            } else {
                &caption[..1]
            }
        } else {
            caption
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
        Mode::Web => (" WEB ", BLUE),
        Mode::Help => (" HELP ", BLUE),
        Mode::Input(input) => (editor_mode(input), AMBER),
        Mode::Smart(_) => (" INSERT ", AMBER),
        Mode::Picker(_) => (" SELECT ", BLUE),
        Mode::Leader => (" SPACE ", AMBER),
        Mode::Document(_) => (" INSPECT ", AMBER),
        Mode::Settings(_) => (" PREVIEW ", AMBER),
        Mode::Image(_) => (" IMAGE ", AMBER),
    };
    let activity = if app.session_failed() {
        "  SAVE ERROR · :save-status"
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
            format!(" {} ", focus_caption(app)),
            Style::new().fg(TEXT).bg(RAISED),
        ),
        Span::styled(activity, Style::new().fg(AMBER)),
        Span::styled(
            format!("  {}", content::label(&app.status, 300)),
            Style::new().fg(MUTED),
        ),
    ])
}

fn editor_mode(input: &crate::app::Input) -> &'static str {
    if matches!(input.kind, InputKind::Command) {
        return " COMMAND ";
    }
    match input.edit_mode {
        EditMode::Insert => " INSERT ",
        EditMode::Normal | EditMode::PendingZ => " NORMAL ",
        EditMode::Command(_) => " COMMAND ",
        EditMode::Search(_) => " SEARCH ",
    }
}

fn focus_caption(app: &App) -> &'static str {
    if app.notebook.is_some() {
        return match app.focus {
            Focus::Library => "Collections",
            Focus::Articles => "Notes",
            Focus::Reader => "Context",
            Focus::Annotations => "Backlinks",
        };
    }
    app.focus.title()
}
