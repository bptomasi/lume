//! Unicode cursor boundaries shared by reader input and presentation.

use unicode_segmentation::{GraphemeCursor, UnicodeSegmentation};
use unicode_width::UnicodeWidthStr;

/// Closest grapheme start at or before a byte offset.
pub fn boundary(text: &str, byte: usize) -> usize {
    let mut offset = byte.min(text.len());
    while !text.is_char_boundary(offset) {
        offset -= 1;
    }
    let mut cursor = GraphemeCursor::new(offset, text.len(), true);
    if offset < text.len() {
        match cursor.is_boundary(text, 0) {
            Ok(true) => return offset,
            Ok(false) => {}
            Err(_) => return scanned_boundary(text, byte),
        }
    }
    match cursor.prev_boundary(text, 0) {
        Ok(Some(boundary)) => boundary,
        Ok(None) => 0,
        // Full context normally resolves the cursor locally. Preserve the
        // original contract if a library context request cannot be satisfied.
        Err(_) => scanned_boundary(text, byte),
    }
}

fn scanned_boundary(text: &str, byte: usize) -> usize {
    text.grapheme_indices(true)
        .map(|(start, _)| start)
        .take_while(|start| *start <= byte)
        .last()
        .unwrap_or(0)
}

/// End of the grapheme at a valid byte boundary, clamped to the text length.
pub fn next(text: &str, byte: usize) -> usize {
    text.get(byte..)
        .and_then(|tail| tail.graphemes(true).next())
        .map_or(text.len(), |grapheme| byte + grapheme.len())
}

/// Beginning of the previous grapheme, including combining marks and emoji.
pub fn previous(text: &str, byte: usize) -> usize {
    text.get(..byte)
        .and_then(|head| head.grapheme_indices(true).next_back())
        .map_or(0, |(start, _)| start)
}

/// Terminal cells occupied before a byte boundary; tabs use four-column stops.
pub fn column(text: &str, byte: usize) -> usize {
    text.get(..byte)
        .unwrap_or_default()
        .graphemes(true)
        .fold(0, |column, grapheme| column + width(grapheme, column))
}

/// Byte boundary at or before a desired terminal column, clamped to the row.
pub fn byte_at(text: &str, goal: usize) -> usize {
    let mut column = 0;
    for (byte, grapheme) in text.grapheme_indices(true) {
        let end = column + width(grapheme, column);
        if end > goal {
            return byte;
        }
        column = end;
    }
    previous(text, text.len())
}

/// Cell width with a consistent tab-stop policy.
pub fn width(grapheme: &str, column: usize) -> usize {
    if grapheme == "\t" {
        return 4 - column % 4;
    }
    grapheme.width()
}

/// Next/previous Unicode word or punctuation boundary, without splitting UTF-8.
pub fn word(text: &str, byte: usize, forward: bool) -> usize {
    let mut boundaries = text
        .split_word_bound_indices()
        .filter(|(_, word)| !word.trim().is_empty())
        .map(|(start, _)| start);
    if forward {
        return boundaries
            .find(|start| *start > byte)
            .unwrap_or_else(|| previous(text, text.len()));
    }
    boundaries.rfind(|start| *start < byte).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use unicode_segmentation::UnicodeSegmentation;

    #[test]
    fn local_boundaries_match_complete_unicode_segmentation_at_every_byte() {
        for text in [
            "",
            "plain ASCII",
            "\r\nnext\r\n",
            "e\u{301}x",
            "\u{301}a",
            "👩‍💻🧑🏽‍🚀",
            "🇧🇷🇨🇦🇺",
            "क्\u{200d}षक्ष",
            "각나",
            "\u{600}a\u{903}b",
            "long context 🇦🇧🇨🇩🇪🇫🇬🇭 e\u{301} 👩‍👩‍👧‍👦",
        ] {
            let starts = text
                .grapheme_indices(true)
                .map(|(byte, _)| byte)
                .collect::<Vec<_>>();
            for offset in 0..=text.len() + 3 {
                let index = starts.partition_point(|start| *start <= offset);
                let expected =
                    index.checked_sub(1).map_or(0, |index| starts[index]);
                assert_eq!(
                    super::boundary(text, offset),
                    expected,
                    "{text:?} at {offset}"
                );
            }
            let expected = starts.last().copied().unwrap_or(0);
            assert_eq!(super::boundary(text, usize::MAX), expected);
        }
    }
}
