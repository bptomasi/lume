//! The dedicated inspector reuses the reader's source-aware table layout.

use ratatui::text::Line;

const MAX_BYTES: usize = super::tables::MAX_BYTES;
const LIMIT_NOTICE: &str =
    "\n[Table preview limited to 1 MiB; the article is unchanged.]\n";

/// Render a horizontally scrollable table grid with a one MiB output budget.
///
/// Oversized or sparse tables fall back to their original Markdown rather than
/// expanding padding or discarding cells. The article itself is never changed.
pub fn render(source: &str) -> String {
    if source.len() > MAX_BYTES {
        return limited(source);
    }
    let rendered = super::render_with(source, 4096, &mut |text, _| {
        text.lines()
            .map(|line| Line::raw(line.to_owned()))
            .collect()
    });
    from_lines(rendered.text.lines())
}

pub(crate) fn from_lines<'a>(
    lines: impl IntoIterator<Item = &'a str>,
) -> String {
    let mut output = String::new();
    for line in lines {
        if output.len() + line.len() + 1 > MAX_BYTES {
            let budget = MAX_BYTES - LIMIT_NOTICE.len();
            output.truncate(output.floor_char_boundary(budget));
            let end = line.floor_char_boundary(budget - output.len());
            output.push_str(&line[..end]);
            output.push_str(LIMIT_NOTICE);
            break;
        }
        output.push_str(line);
        output.push('\n');
    }
    output
}

fn limited(text: &str) -> String {
    let end = text.floor_char_boundary(MAX_BYTES - LIMIT_NOTICE.len());
    let mut output = text[..end].to_owned();
    output.push_str(LIMIT_NOTICE);
    output
}
