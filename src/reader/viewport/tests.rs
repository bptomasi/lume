//! Unicode, tab stops and hidden syntax spans retain the prior cell contract.

use super::RowIndex;
use crate::reader::cursor;
use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};
use unicode_segmentation::UnicodeSegmentation;

#[test]
fn sparse_checkpoints_match_complete_unicode_cell_mapping() {
    let text = format!("{}END", "a\t界e\u{301}👩‍💻🇧🇷 ".repeat(300));
    let line = Line::raw(text.as_str());
    let index = RowIndex::new(&text, &line);
    for (byte, _) in text.grapheme_indices(true) {
        assert_eq!(index.column(&text, byte), cursor::column(&text, byte));
    }
    for column in 0..cursor::column(&text, text.len()) + 3 {
        assert_eq!(
            index.byte_at(&text, column),
            cursor::byte_at(&text, column)
        );
    }
}

#[test]
fn deep_projection_preserves_styles_decorations_and_partial_wide_cells() {
    let prefix = "a".repeat(100_000);
    let text = format!("{prefix}\t界ABC");
    let line = Line::from(vec![
        Span::styled(prefix, Style::new().fg(Color::Red)),
        Span::styled("\t界ABC", Style::new().fg(Color::Blue)),
    ]);
    let index = RowIndex::new(&text, &line);

    let clipped = index.project(&line, &text, 100_003, 5, |bytes, style| {
        if bytes.start >= 100_004 {
            style.bg(Color::Yellow)
        } else {
            style
        }
    });

    assert_eq!(clipped.to_string(), " 界AB");
    assert_eq!(clipped.width(), 5);
    assert_eq!(clipped.spans[0].style.fg, Some(Color::Blue));
    assert_eq!(
        clipped.spans.last().and_then(|span| span.style.bg),
        Some(Color::Yellow)
    );
    assert_eq!(
        index
            .project(&line, &text, usize::MAX, 80, |_, style| style)
            .width(),
        0
    );
}

#[test]
fn justification_expands_visual_gaps_without_changing_unicode_byte_identity() {
    let text = "A café 界 end";
    let line = ratatui::text::Line::raw(text);
    let index = super::RowIndex::justified(text, &line, 17);

    let projected = index.project(&line, text, 0, 17, |_, style| style);

    assert_eq!(projected.to_string(), "A   café  界  end");
    assert_eq!(projected.width(), 17);
    assert_eq!(index.column(text, 2), 4);
    for column in 1..4 {
        assert_eq!(index.byte_at(text, column), 1);
    }
    assert_eq!(index.byte_at(text, 4), 2);
    assert_eq!(text, "A café 界 end");
}

#[test]
fn list_insets_are_visual_and_use_the_same_mouse_and_cursor_coordinates() {
    let text = "café 界";
    let line = Line::raw(text);
    let index = RowIndex::inset(text, &line, 3);

    assert_eq!(index.column(text, 0), 3);
    for column in 0..4 {
        assert_eq!(index.byte_at(text, column), 0);
    }
    assert_eq!(index.column(text, 6), 8);
    assert_eq!(index.byte_at(text, 9), 6);
    assert_eq!(
        index
            .project(&line, text, 0, 10, |_, style| style)
            .to_string(),
        "   café 界"
    );
    assert_eq!(
        index
            .project(&line, text, 2, 5, |_, style| style)
            .to_string(),
        " café"
    );
    assert_eq!(
        index
            .project(&line, text, 0, 2, |_, style| style)
            .to_string(),
        "  "
    );
}
