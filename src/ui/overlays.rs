//! Modal editors, a scrollable field guide and contextual leader actions.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::Line,
    widgets::{List, ListState, Paragraph},
};

use crate::app::{App, EditMode, HELP, Input, InputKind, Mode};

use super::{popup::Popup, selectors, theme::*};

pub(super) fn draw(frame: &mut Frame<'_>, app: &App) {
    match &app.mode {
        Mode::Help => help(frame, app),
        Mode::Input(input) => editor(frame, app, input),
        Mode::ServerToken(input) => server_token(frame, input),
        Mode::Picker(picker) => selectors::picker(frame, picker),
        Mode::Smart(form) => selectors::smart(frame, form),
        Mode::Document(document) => document_view(frame, document),
        Mode::Settings(draft) => settings(frame, app, draft),
        Mode::Leader => leader(frame),
        Mode::Normal | Mode::Web => {}
        Mode::Image(_) => {}
    }
}

fn settings(
    frame: &mut Frame<'_>,
    app: &App,
    draft: &crate::app::SettingsDraft,
) {
    let mut inner = modal(
        frame,
        "Settings · Enter edit · Ctrl-s save · Esc discard",
        Popup::Settings,
    );
    if let Some(editor) = &draft.editor {
        let height = inner.height.min(2);
        let y = inner.bottom().saturating_sub(height);
        frame.render_widget(
            editor,
            Rect::new(inner.x, y, inner.width, height.min(1)),
        );
        if height > 1 {
            frame.render_widget(
                Paragraph::new("Enter applies value · Esc cancels edit"),
                Rect::new(inner.x, y + 1, inner.width, 1),
            );
        }
        inner.height = inner.height.saturating_sub(height);
    }
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
        format!("Justify prose: {}", app.workspace.justify),
        format!("LaTeX formulas: {}", app.workspace.latex_formulas),
        format!(
            "Formula presentation: {}",
            if app.workspace.latex_graphics {
                "Graphics"
            } else {
                "Text"
            }
        ),
        format!("Automatic read state: {}", app.workspace.auto_read.enabled),
        format!(
            "Active reading seconds: {}",
            app.workspace.auto_read.min_active_seconds
        ),
        format!(
            "Displayed article coverage: {}%",
            app.workspace.auto_read.min_coverage_percent
        ),
    ]
}

fn document_view(frame: &mut Frame<'_>, document: &crate::app::DocumentView) {
    let inner = modal(frame, &document.title, Popup::Document);
    let lines = document
        .lines
        .iter()
        .skip(document.row)
        .take(inner.height as usize)
        .map(|line| {
            Line::raw(clip_columns(line, document.column, inner.width.into()))
        })
        .collect::<Vec<_>>();
    frame.render_widget(
        Paragraph::new(lines).style(Style::new().fg(TEXT)),
        inner,
    );
}

fn clip_columns(text: &str, skip: usize, width: usize) -> String {
    use unicode_segmentation::UnicodeSegmentation;
    let mut column = 0;
    let mut output = String::new();
    let limit = skip.saturating_add(width);
    for grapheme in text.graphemes(true) {
        if column >= limit {
            break;
        }
        let end = column + crate::reader::cursor::width(grapheme, column);
        if end > skip {
            if column < skip || end > limit || grapheme == "\t" {
                output.push_str(&" ".repeat(end.min(limit) - column.max(skip)));
            } else {
                output.push_str(grapheme);
            }
        }
        column = end;
    }
    output
}

fn help(frame: &mut Frame<'_>, app: &App) {
    let mut inner = modal(
        frame,
        "Lume · field guide · / search · j/k scroll · Esc close",
        Popup::Help,
    );
    if app.help_search.editor.is_some() || !app.help_search.query.is_empty() {
        inner = help_search_field(frame, app, inner);
    }
    let capacity = inner.height as usize;
    let offset = app.help_scroll.min(HELP.lines().count().saturating_sub(1));
    let lines = HELP
        .lines()
        .skip(offset)
        .take(capacity)
        .enumerate()
        .map(|(row, line)| {
            let style = if app
                .help_search
                .hits
                .binary_search(&(offset + row))
                .is_ok()
            {
                Style::new()
                    .fg(AMBER)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
            } else if line.starts_with("  ─") {
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

fn help_search_field(frame: &mut Frame<'_>, app: &App, inner: Rect) -> Rect {
    let field = Rect::new(inner.x, inner.y, inner.width, inner.height.min(1));
    frame.render_widget(
        Paragraph::new("/ ").style(Style::new().fg(AMBER)),
        field,
    );
    let editor = Rect::new(
        field.x + field.width.min(2),
        field.y,
        field.width.saturating_sub(2),
        field.height,
    );
    if let Some(input) = &app.help_search.editor {
        frame.render_widget(input, editor);
    } else {
        frame.render_widget(
            Paragraph::new(app.help_search.query.as_str()),
            editor,
        );
    }
    if inner.height > 1 {
        let count = app.help_search.hits.len();
        let current = if count == 0 {
            0
        } else {
            app.help_search.selected + 1
        };
        frame.render_widget(
            Paragraph::new(format!(
                "{current}/{count} matches · n/N next/previous · Esc clear"
            ))
            .style(Style::new().fg(MUTED)),
            Rect::new(inner.x, inner.y + 1, inner.width, 1),
        );
    }
    let used = inner.height.min(2);
    Rect::new(inner.x, inner.y + used, inner.width, inner.height - used)
}

fn editor(frame: &mut Frame<'_>, app: &App, input: &Input) {
    if matches!(input.kind, InputKind::Search | InputKind::LibrarySearch) {
        return;
    }
    if matches!(input.kind, InputKind::Command) {
        super::command_line::draw(frame, app, input);
        return;
    }
    let multiline = super::popup::multiline(&input.kind);
    let title = format!(
        "{}{}",
        editor_title(&input.kind),
        if multiline && input.modified() {
            " [+]"
        } else {
            ""
        },
    );
    let inner = modal(frame, &title, Popup::Editor(multiline));
    let editor_area = Rect::new(
        inner.x + 1,
        inner.y + 1,
        inner.width.saturating_sub(2),
        inner.height.saturating_sub(5),
    );
    frame.render_widget(&input.editor, editor_area);
    editor_footer(frame, app, input, inner, multiline);
}

fn editor_footer(
    frame: &mut Frame<'_>,
    app: &App,
    input: &Input,
    inner: Rect,
    multiline: bool,
) {
    let hint = editor_hint(input, multiline);
    let command = match &input.edit_mode {
        EditMode::Command(line) => format!(":{line}█"),
        EditMode::Search(line) => format!("/{line}█"),
        _ => String::new(),
    };
    let footer = Rect::new(
        inner.x + 1,
        inner.bottom().saturating_sub(3),
        inner.width.saturating_sub(2),
        3,
    );
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled(
                crate::content::label(&app.status, 160),
                Style::new().fg(MUTED),
            ),
            Line::styled(command, Style::new().fg(TEXT)),
            Line::styled(hint, Style::new().fg(AMBER)),
        ]),
        footer,
    );
}

pub(super) fn editor_hint(input: &Input, multiline: bool) -> &'static str {
    if !multiline {
        return "Enter apply · arrows move cursor · Ctrl-u clear · Esc cancel";
    }
    match input.edit_mode {
        EditMode::Insert => {
            "INSERT · Esc normal · Ctrl-s save/close · Ctrl-e external"
        }
        EditMode::Normal => {
            "NORMAL · i insert · :w save · :wq close · :wqa quit · :q! discard"
        }
        EditMode::PendingZ => {
            "Z · Z write and close · Q discard and close · Esc cancel"
        }
        EditMode::Search(_) => {
            "SEARCH · Enter finds / highlights · n next · N previous · Esc cancel"
        }
        EditMode::Command(_) => {
            "COMMAND · :w save · :wq close · :wqa quit · :q! discard · Esc cancel"
        }
    }
}

fn editor_title(kind: &InputKind) -> &'static str {
    match kind {
        InputKind::NoteTags { .. } => {
            "Note tags · space separated · Enter saves"
        }
        InputKind::NoteCollection => {
            "Save note collection · name for the current query"
        }
        InputKind::NoteSearch => {
            "Search notes · tags, type, status, source · Tab completes"
        }
        InputKind::Search => "Search · titles, bodies and annotations",
        InputKind::LibrarySearch => "Search library destinations",
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
    let inner = modal(frame, "Space · choose an action", Popup::Leader);
    frame.render_widget(
        Paragraph::new(LEADER).style(Style::new().fg(TEXT)),
        inner,
    );
}

const LEADER: &str = "
 p  Command palette         f  Find an article
 l  Organize in folders       n  Your notebook
 q  Read-next queue         z  Zen reading
 Z  Fullscreen article     R  For you
 b  Toggle library         a  Toggle headlines
 v  Toggle reader          t  Article annotations
 c  Compact headlines      s  Smart list
 d  Discover feeds         i  Import OPML
 e  Export article         o  Article links
 u  Undo personal edit     r  Collect feeds

 Esc cancel · Ctrl-p opens the palette directly
";

pub(super) fn modal(frame: &mut Frame<'_>, title: &str, popup: Popup) -> Rect {
    super::popup::draw_frame(frame, title, popup.area(frame.area()))
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

fn server_token(frame: &mut Frame<'_>, input: &crate::app::ServerToken) {
    let inner = modal(
        frame,
        "API token · Enter saves to TOML · Esc cancels",
        Popup::Token,
    );
    let masked = input.masked(inner.width);
    if !inner.is_empty() && inner.height > 1 {
        frame.set_cursor_position((
            inner.x + (masked.chars().count() as u16).min(inner.width - 1),
            inner.y + 1,
        ));
    }
    frame.render_widget(
        Paragraph::new(vec![
            Line::raw(input.address()),
            Line::raw(masked),
            Line::raw("Ctrl-u clears the key · Backspace edits"),
        ]),
        inner,
    );
}

#[cfg(test)]
mod tests {
    #[test]
    fn document_clipping_bounds_work_and_keeps_partial_wide_cells_aligned() {
        assert_eq!(super::clip_columns("A界B", 2, 2), " B");
        assert_eq!(super::clip_columns("A界B", 0, 2), "A ");
        assert_eq!(super::clip_columns("\txy", 1, 4), "   x");
        assert_eq!(super::clip_columns(&"a".repeat(100_000), 0, 8), "aaaaaaaa");
    }
}
