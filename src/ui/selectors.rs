//! Search selectors with a bounded cached preview and typed smart-list fields.

use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::Line,
    widgets::{Paragraph, Wrap},
};

use crate::app::{ChoiceAction, Picker, PickerKind, SmartForm};

use super::{overlays::modal, popup::Popup, theme::*};

pub(super) fn picker(frame: &mut Frame<'_>, picker: &Picker) {
    let title = picker_title(&picker.kind);
    let inner = modal(frame, title, Popup::Picker);
    frame.render_widget(
        Paragraph::new(format!(" › {}▎", picker.query))
            .style(Style::new().fg(AMBER)),
        Rect::new(inner.x, inner.y, inner.width, 2),
    );
    let (choices_area, preview_area) = super::picker_regions(frame.area());
    choices(frame, picker, choices_area);
    frame.render_widget(
        Paragraph::new(picker.preview.as_str())
            .style(Style::new().fg(MUTED))
            .wrap(Wrap { trim: false }),
        preview_area,
    );
    frame.render_widget(
        Paragraph::new(" ↑↓ / Ctrl-j/k select · Enter apply · Esc cancel")
            .style(Style::new().fg(AMBER)),
        Rect::new(inner.x, inner.bottom().saturating_sub(1), inner.width, 1),
    );
}

fn choices(frame: &mut Frame<'_>, picker: &Picker, area: Rect) {
    let capacity = area.height as usize;
    let first = picker.selected.saturating_sub(capacity / 2);
    let lines = picker
        .choices
        .iter()
        .enumerate()
        .skip(first)
        .take(capacity)
        .map(|(index, choice)| {
            let marker = match (&picker.kind, &choice.action) {
                (
                    PickerKind::Lists { checked, .. },
                    ChoiceAction::Membership(id),
                ) => {
                    if checked.contains(id) {
                        " ☑ "
                    } else {
                        " ☐ "
                    }
                }
                _ if index == picker.selected => " › ",
                _ => "   ",
            };
            let style = if index == picker.selected {
                Style::new().fg(AMBER).bg(RAISED)
            } else {
                Style::new().fg(TEXT)
            };
            Line::styled(format!("{marker}{}", choice.title), style)
        })
        .collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(lines), area);
}

pub(super) fn smart(frame: &mut Frame<'_>, form: &SmartForm) {
    let title = if form.feeds.is_empty() {
        "Smart list · all criteria must match".into()
    } else {
        format!("Smart list · {} grouped sources retained", form.feeds.len())
    };
    let inner = modal(frame, &title, Popup::Smart);
    if inner.height < 18 || inner.width < 60 {
        compact_smart(frame, form, inner);
        return;
    }
    for (index, label) in SMART_FIELDS.iter().enumerate() {
        smart_field(frame, form, inner, index, label);
    }
    for (index, (label, checked)) in [
        ("Unread only", form.unread),
        ("Favorites only", form.starred),
    ]
    .iter()
    .enumerate()
    {
        let color = if form.field == index + 4 { AMBER } else { TEXT };
        frame.render_widget(
            Paragraph::new(format!(
                " {} {label}",
                if *checked { "☑" } else { "☐" }
            ))
            .style(Style::new().fg(color)),
            Rect::new(inner.x, inner.y + 12 + index as u16, inner.width, 1),
        );
    }
    frame.render_widget(
        Paragraph::new(
            " Tab next field · Space toggle · Ctrl-s save · Esc cancel",
        )
        .style(Style::new().fg(AMBER)),
        Rect::new(inner.x, inner.bottom().saturating_sub(1), inner.width, 1),
    );
}

fn picker_title(kind: &PickerKind) -> &'static str {
    match kind {
        PickerKind::Commands => "Command palette",
        PickerKind::Articles => "Find an article · full text",
        PickerKind::Folders => {
            "Your folders · Enter opens · New folder creates"
        }
        PickerKind::Lists { .. } => "Folders · Space select · Ctrl-n create",
        PickerKind::Links => "Article links",
        PickerKind::Sources => "Choose a subscription",
        PickerKind::Inspection => "Inspect · Enter applies · Esc closes",
    }
}

const SMART_FIELDS: &[&str] = &[
    "Name",
    "Full-text terms",
    "Source name or feed URL (optional)",
    "Published within days (optional)",
];

fn smart_field(
    frame: &mut Frame<'_>,
    form: &SmartForm,
    inner: Rect,
    index: usize,
    label: &str,
) {
    let y = inner.y + index as u16 * 3;
    frame.render_widget(
        Paragraph::new(label).style(Style::new().fg(MUTED)),
        Rect::new(inner.x + 1, y, inner.width.saturating_sub(2), 1),
    );
    let mut editor = form.fields[index].clone();
    editor.set_style(Style::new().fg(TEXT).bg(RAISED));
    editor.set_cursor_line_style(Style::default());
    editor.set_cursor_style(if form.field == index {
        Style::new().fg(BG).bg(AMBER)
    } else {
        Style::default()
    });
    frame.render_widget(
        &editor,
        Rect::new(inner.x + 1, y + 1, inner.width.saturating_sub(2), 1),
    );
}

fn compact_smart(frame: &mut Frame<'_>, form: &SmartForm, area: Rect) {
    let label = match form.field {
        0..=3 => SMART_FIELDS[form.field],
        4 => "Unread only · Space toggles",
        _ => "Favorites only · Space toggles",
    };
    frame.render_widget(
        Paragraph::new(format!(" {}/6 · {label}", form.field + 1))
            .style(Style::new().fg(AMBER)),
        Rect::new(area.x, area.y, area.width, 1),
    );
    let body = Rect::new(
        area.x + 1,
        area.y + 2,
        area.width.saturating_sub(2),
        area.height.saturating_sub(4),
    );
    if form.field < 4 {
        frame.render_widget(&form.fields[form.field], body);
    } else {
        let checked = if form.field == 4 {
            form.unread
        } else {
            form.starred
        };
        frame.render_widget(
            Paragraph::new(if checked {
                "☑ Enabled"
            } else {
                "☐ Disabled"
            }),
            body,
        );
    }
    frame.render_widget(
        Paragraph::new("Tab next · Ctrl-s save · Esc cancel")
            .style(Style::new().fg(MUTED)),
        Rect::new(area.x, area.bottom().saturating_sub(1), area.width, 1),
    );
}
