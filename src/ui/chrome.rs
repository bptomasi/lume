//! Global identity, pane controls and the modal status line.

use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::{
    app::{ChromeView, ControlsView, EditMode, Focus, InputKind, Mode},
    content,
};

use super::theme::*;

pub(super) fn header(frame: &mut Frame<'_>, view: &ChromeView<'_>) {
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
    let state = if view.demo {
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

pub(super) fn footer(frame: &mut Frame<'_>, view: &ChromeView<'_>) {
    let area = frame.area();
    if area.height < 2 {
        return;
    }
    let status = status_line(view);
    frame.render_widget(
        Paragraph::new(status),
        Rect::new(0, area.height - 2, area.width, 1),
    );
    frame.render_widget(
        Paragraph::new(hints(view)).style(Style::new().fg(FAINT)),
        Rect::new(0, area.height - 1, area.width, 1),
    );
}

fn hints(view: &ChromeView<'_>) -> &'static str {
    if let Mode::Input(input) = view.mode
        && matches!(
            input.kind,
            InputKind::Comment { .. }
                | InputKind::Highlight { .. }
                | InputKind::StudyCard { .. }
        )
    {
        return super::overlays::editor_hint(input, true);
    }
    if view.layout_prefix_pending {
        return "  Ctrl-w · h/j/k/l focus · H/J/K/L move · s/v split · u/r history · Esc cancel";
    }
    if view.notebook {
        return "  / notes · e edit · Enter source · t tags · T type · p pin · r resolve · v mark · s synthesize · dd / u · Esc return";
    }
    if view.focus == Focus::Annotations {
        return concat!(
            "  j/k notes · Enter jump · e edit · dd delete · ",
            "Ctrl-w c collapse · Reader n creates"
        );
    }
    if view.focus == Focus::Reader {
        return reader_hints(view);
    }
    concat!(
        "  space actions  ·  / search  ·  f star  ·  s queue  ·  ",
        "Space / notes · Z fullscreen · ? help"
    )
}

fn reader_hints(view: &ChromeView<'_>) -> &'static str {
    if view.selection {
        return "  y copy  ·  s highlight  ·  n note  ·  Esc cancel";
    }
    if !view.preparing
        && view
            .reader
            .note_at_offset(view.reader.cursor_offset())
            .is_some()
    {
        return "  Enter edit · dd delete · u undo · :annotations · Tab panels";
    }
    if !view.reader_search {
        return "  w/W/e word · / find · n note · :annotate · Ctrl-w layout";
    }
    concat!(
        "  w/W/e word  ·  / find  ·  n/N match  ·  :note  ·  ",
        "Z fullscreen  ·  ? help"
    )
}

pub(super) fn pane_controls(
    frame: &mut Frame<'_>,
    view: &ControlsView<'_>,
    areas: &super::Areas,
) {
    for (pane, area) in areas.ordered() {
        move_grip(frame, view, pane, area);
        if let Some(button) = super::pane_close_rect(area) {
            frame.render_widget(
                Paragraph::new("[−]").style(Style::new().fg(AMBER).bg(PANEL)),
                button,
            );
        }
    }
    if view.workspace.fullscreen {
        return;
    }
    restore_buttons(frame, view);
    if areas.is_empty() {
        collapsed_workspace(frame);
    }
}

fn move_grip(
    frame: &mut Frame<'_>,
    view: &ControlsView<'_>,
    pane: Focus,
    area: Rect,
) {
    let Some(grip) = super::pane_move_rect(area) else {
        return;
    };
    // Reserve the controls' entire strip, including their one-cell separator.
    // A long title must not leave stray letters between the two buttons.
    let strip = Rect::new(grip.x, grip.y, 7, 1);
    let border = if view.focus == pane { ACCENT } else { BORDER };
    frame.render_widget(
        Block::new()
            .borders(Borders::TOP)
            .style(Style::new().bg(PANEL))
            .border_style(Style::new().fg(border)),
        strip,
    );
    let moving = view.panel_drag.is_some_and(|drag| drag.pane == pane);
    let style = if moving {
        Style::new().fg(BG).bg(AMBER)
    } else {
        Style::new().fg(AMBER).bg(PANEL)
    };
    frame.render_widget(Paragraph::new("[≡]").style(style), grip);
}

pub(super) fn draw_panel_move(frame: &mut Frame<'_>, view: &ControlsView<'_>) {
    let Some(drag) = view.panel_drag else {
        return;
    };
    let areas =
        super::workspace_areas(frame.area(), view.focus, view.workspace);
    for (pane, area) in areas.ordered() {
        // Only decorate destinations. The source is captured intact, with
        // its original rounded corners, title and controls.
        if drag.target != Some(pane) {
            continue;
        }
        let area = if drag.edge.is_some() {
            area
        } else {
            view.pane_insertion_slot.unwrap_or(area)
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

fn restore_buttons(frame: &mut Frame<'_>, view: &ControlsView<'_>) {
    for (pane, button) in super::pane_buttons(frame.area(), view.workspace) {
        let visible = view.workspace.pane_visible(pane);
        let caption = if view.notebook {
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
        let style = if view
            .panel_drag
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

fn status_line(view: &ChromeView<'_>) -> Line<'static> {
    let (mode, color) = match view.mode {
        Mode::Normal if view.selection => (" VISUAL ", AMBER),
        Mode::Normal => (" NORMAL ", ACCENT),
        Mode::Web => (" WEB ", BLUE),
        Mode::Help => (" HELP ", BLUE),
        Mode::Input(input) => (editor_mode(input), AMBER),
        Mode::Smart(_) => (" INSERT ", AMBER),
        Mode::ServerToken(_) => (" TOKEN ", AMBER),
        Mode::Picker(_) => (" SELECT ", BLUE),
        Mode::Leader => (" SPACE ", AMBER),
        Mode::Document(_) => (" INSPECT ", AMBER),
        Mode::Settings(_) => (" PREVIEW ", AMBER),
        Mode::Image(_) => (" IMAGE ", AMBER),
    };
    let activity = if view.session_failed {
        "  SAVE ERROR · :save-status"
    } else if view.session_pending {
        "  saving…"
    } else if view.refreshing {
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
            format!(" {} ", focus_caption(view)),
            Style::new().fg(TEXT).bg(RAISED),
        ),
        Span::styled(activity, Style::new().fg(AMBER)),
        Span::styled(
            format!("  {}", content::label(view.status, 300)),
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

fn focus_caption(view: &ChromeView<'_>) -> &'static str {
    if view.notebook {
        return match view.focus {
            Focus::Library => "Collections",
            Focus::Articles => "Notes",
            Focus::Reader => "Context",
            Focus::Annotations => "Backlinks",
        };
    }
    view.focus.title()
}
