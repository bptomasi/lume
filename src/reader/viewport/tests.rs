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
