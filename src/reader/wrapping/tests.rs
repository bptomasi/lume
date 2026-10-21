//! Independent byte fixtures exercise gaps, empty spans, entities and UTF-8.

use ratatui::{
    style::{Color, Style},
    text::Span,
};

use super::Paragraph;
use crate::reader::SourceMapping;

#[test]
fn sequential_rows_keep_exact_styles_and_source_after_discarded_spaces() {
    let spans = [
        Span::styled("αβ ", Style::new().fg(Color::Red)),
        Span::raw(""),
        Span::styled("fish", Style::new().fg(Color::Blue)),
        Span::raw("&"),
        Span::styled(" tail", Style::new().fg(Color::Green)),
    ];
    let mappings = [
        SourceMapping::new(0..5, 10..15),
        SourceMapping::new(5..9, 20..24),
        SourceMapping::new(9..10, 30..35),
        SourceMapping::new(10..15, 40..45),
    ];
    let mut paragraph = Paragraph::new(&spans, &mappings);

    let (first, source) = paragraph.row(0..4);
    assert_eq!(first.to_string(), "αβ");
    assert_eq!(source, Some(10..14));
    assert_eq!(first.spans[0].style.fg, Some(Color::Red));
    let (middle, source) = paragraph.row(5..10);
    assert_eq!(middle.to_string(), "fish&");
    assert_eq!(source, Some(20..35));
    assert_eq!(middle.spans[0].style.fg, Some(Color::Blue));
    let (last, source) = paragraph.row(11..15);
    assert_eq!(last.to_string(), "tail");
    assert_eq!(source, Some(41..45));
    assert_eq!(last.spans[0].style.fg, Some(Color::Green));
}
