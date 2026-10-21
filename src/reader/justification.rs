//! Visual word-gap expansion never modifies readable text or source mappings.
//!
//! textwrap owns line breaking; this policy expands only existing ASCII word
//! gaps in eligible prose rows. A gap grows by at most three cells, avoiding
//! sparse, stretched lines. Unicode cells remain owned by the cursor library.

use super::cursor;

pub(super) fn gaps(text: &str, width: usize) -> Vec<(usize, usize)> {
    if text.contains('\t') {
        return Vec::new();
    }
    let remaining = width.saturating_sub(cursor::column(text, text.len()));
    let gaps = text
        .match_indices(' ')
        .filter(|(byte, _)| {
            *byte > 0
                && *byte + 1 < text.len()
                && text.as_bytes()[byte - 1] != b' '
                && text.as_bytes()[byte + 1] != b' '
        })
        .map(|(byte, _)| byte)
        .collect::<Vec<_>>();
    if gaps.is_empty() || remaining == 0 || remaining > gaps.len() * 3 {
        return Vec::new();
    }
    let each = remaining / gaps.len();
    let extra = remaining % gaps.len();
    gaps.into_iter()
        .enumerate()
        .map(|(index, byte)| (byte, each + usize::from(index < extra)))
        .filter(|(_, padding)| *padding > 0)
        .collect()
}
