//! Modal editors, a scrollable field guide and contextual leader actions.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::Line,
    widgets::{Clear, List, ListState, Paragraph},
};

use crate::app::{App, HELP, Input, InputKind, Mode};

use super::{selectors, theme::*};

pub(super) fn draw(frame: &mut Frame<'_>, app: &App) {
    match &app.mode {
        Mode::Help => help(frame, app),
        Mode::Input(input) => editor(frame, app, input),
        Mode::Picker(picker) => selectors::picker(frame, picker),
        Mode::Smart(form) => selectors::smart(frame, form),
        Mode::Document(document) => document_view(frame, document),
        Mode::Settings(draft) => settings(frame, app, draft),
        Mode::Leader => leader(frame),
        Mode::Normal => {}
    }
}

fn settings(
    frame: &mut Frame<'_>,
    app: &App,
    draft: &crate::app::SettingsDraft,
) {
    let inner = modal(
        frame,
        "Visual preview · ↑↓ field · ←→ change · Enter save · Esc discard",
        96,
        15,
    );
    let lines = visual_preferences(app)
        .into_iter()
        .enumerate()
        .map(|(index, value)| {
            Line::styled(
                format!(
                    " {} {value}",
                    if index == draft.field { "›" } else { " " }
                ),
                Style::new().fg(if index == draft.field {
                    AMBER
                } else {
                    TEXT
                }),
            )
        })
        .collect::<Vec<_>>();
    let mut state = ListState::default().with_selected(Some(draft.field));
    frame.render_stateful_widget(List::new(lines), inner, &mut state);
}

fn visual_preferences(
    app: &App,
) -> [String; crate::app::SettingsDraft::FIELD_COUNT] {
    [
        format!("Theme: {}", app.workspace.theme),
        format!("Reading width: {}", app.workspace.reading_width),
        format!("Sidebar width: {}", app.workspace.library_width),
        format!("Headlines width: {}", app.workspace.article_width),
        format!("Compact headlines: {}", app.workspace.compact),
        format!("Show library: {}", app.workspace.show_library),
        format!("Show headlines: {}", app.workspace.show_articles),
        format!("Annotations width: {}", app.workspace.annotation_width),
        format!("Show reader: {}", app.workspace.show_reader),
        format!("Show annotations: {}", app.workspace.show_annotations),
        format!("Mouse reporting: {}", app.workspace.mouse),
        format!("Fullscreen: {} cols", app.workspace.fullscreen_width),
    ]
}

fn document_view(frame: &mut Frame<'_>, document: &crate::app::DocumentView) {
    let inner = modal(frame, &document.title, u16::MAX, u16::MAX);
    let lines = document
        .lines
        .iter()
        .skip(document.row)
        .take(inner.height as usize)
        .map(|line| Line::raw(clip_columns(line, document.column)))
        .collect::<Vec<_>>();
    frame.render_widget(
        Paragraph::new(lines).style(Style::new().fg(TEXT)),
        inner,
    );
}

fn clip_columns(text: &str, skip: usize) -> String {
    use unicode_segmentation::UnicodeSegmentation;
    let mut column = 0;
    let mut output = String::new();
    for grapheme in text.graphemes(true) {
        let end = column + crate::reader::cursor::width(grapheme, column);
        if column >= skip {
            output.push_str(grapheme);
        } else if end > skip {
            output.push_str(&" ".repeat(end - skip));
        }
        column = end;
    }
    output
}

fn help(frame: &mut Frame<'_>, app: &App) {
    let inner =
        modal(frame, "Lume · field guide · j/k scroll · Esc close", 88, 40);
    let capacity = inner.height as usize;
    let offset = app.help_scroll.min(HELP.lines().count().saturating_sub(1));
    let lines = HELP
        .lines()
        .skip(offset)
        .take(capacity)
        .map(|line| {
            let style = if line.starts_with("  ─") {
                Style::new().fg(AMBER).add_modifier(Modifier::BOLD)
            } else {
                Style::new().fg(TEXT)
            };
            Line::styled(line, style)
        })
        .collect::<Vec<_>>();
    frame.render_widget(
        Paragraph::new(lines).wrap(ratatui::widgets::Wrap { trim: false }),
        inner,
    );
}

fn editor(frame: &mut Frame<'_>, app: &App, input: &Input) {
    if matches!(input.kind, InputKind::Command) {
        super::command_line::draw(frame, app, input);
        return;
    }
    let multiline = matches!(
        input.kind,
        InputKind::Comment { .. }
            | InputKind::Highlight { .. }
            | InputKind::StudyCard { .. }
    );
    let inner = modal(
        frame,
        editor_title(&input.kind),
        84,
        if multiline { 24 } else { 9 },
    );
    let editor_area = Rect::new(
        inner.x + 1,
        inner.y + 1,
        inner.width.saturating_sub(2),
        inner.height.saturating_sub(4),
    );
    frame.render_widget(&input.editor, editor_area);
    editor_footer(frame, app, inner, multiline);
}

fn editor_footer(
    frame: &mut Frame<'_>,
    app: &App,
    inner: Rect,
    multiline: bool,
) {
    let hint = if multiline {
        "Ctrl-s save · Ctrl-e external editor · Ctrl-z undo · Esc discard"
    } else {
        "Enter apply · arrows move cursor · Ctrl-u clear · Esc cancel"
    };
    let footer = Rect::new(
        inner.x + 1,
        inner.bottom().saturating_sub(2),
        inner.width.saturating_sub(2),
        2,
    );
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled(
                crate::content::label(&app.status, 160),
                Style::new().fg(MUTED),
            ),
            Line::styled(hint, Style::new().fg(AMBER)),
        ]),
        footer,
    );
}

fn editor_title(kind: &InputKind) -> &'static str {
    match kind {
        InputKind::Search => "Search · titles, bodies and annotations",
        InputKind::ReaderSearch => {
            "Find in article · exact text · Enter searches"
        }
        InputKind::Command => ": Command",
        InputKind::NewList => "New article folder",
        InputKind::AddToList(_) => "Save to folder",
        InputKind::Comment { .. } => "Private comment",
        InputKind::Highlight { .. } => "Annotate captured passage",
        InputKind::Discover => "Discover feeds · website or feed URL",
        InputKind::Import => "Import subscriptions · OPML file path",
        InputKind::ExportArticle(_) => {
            "Export article · new Markdown file path"
        }
        InputKind::ExportNotebook => "Export notebook · new Markdown file path",
        InputKind::ExportOpml => "Export subscriptions · new OPML file path",
        InputKind::StudyCard { .. } => {
            "Study card · Question | Answer · Ctrl-s saves"
        }
    }
}

fn leader(frame: &mut Frame<'_>) {
    let inner = modal(frame, "Space · choose an action", 72, 15);
    frame.render_widget(
        Paragraph::new(LEADER).style(Style::new().fg(TEXT)),
        inner,
    );
}

const LEADER: &str = "
 p  Command palette         f  Find an article
 l  Organize in folders       n  Your notebook
 q  Read-next queue         z  Zen reading
 Z  Fullscreen article
 b  Toggle library         a  Toggle headlines
 v  Toggle reader          t  Article annotations
 c  Compact headlines      s  Smart list
 d  Discover feeds         i  Import OPML
 e  Export article         o  Article links
 u  Undo personal edit     r  Collect feeds

 Esc cancel · Ctrl-p opens the palette directly
";

pub(super) fn modal(
    frame: &mut Frame<'_>,
    title: &str,
    width: u16,
    height: u16,
) -> Rect {
    let rect = centered(frame.area(), width, height);
    frame.render_widget(Clear, rect);
    let block = panel(format!(" {title} "), true);
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    inner
}

pub(super) fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width.saturating_sub(2));
    let height = height.min(area.height.saturating_sub(2));
    Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    )
}
