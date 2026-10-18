//! CommonMark cell events drive bounded, aligned inspection of wide tables.

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use unicode_width::UnicodeWidthStr;

const MAX_BYTES: usize = 1024 * 1024;
const LIMIT_NOTICE: &str =
    "\n[Table preview limited to 1 MiB; the article is unchanged.]\n";

/// Render table cells without wrapping; output is limited to one MiB.
///
/// Padding is capped at 80 columns per cell. Longer text remains intact and
/// horizontally scrollable; sparse rows never allocate unbounded padding.
pub fn render(source: &str) -> String {
    let Some(rows) = cells(source) else {
        return unaligned_preview(source);
    };
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0).min(64);
    let widths = (0..columns)
        .map(|column| {
            rows.iter()
                .filter_map(|row| row.get(column))
                .map(|cell| cell.width().min(80))
                .max()
                .unwrap_or(0)
        })
        .collect::<Vec<_>>();
    let mut output = String::new();
    for row in &rows {
        let mut line = String::new();
        for (column, cell) in row.iter().take(columns).enumerate() {
            line.push_str("│ ");
            line.push_str(cell);
            line.push_str(
                &" ".repeat(widths[column].saturating_sub(cell.width())),
            );
            line.push(' ');
        }
        line.push_str("│\n");
        if output.len() + line.len() > MAX_BYTES {
            append_limited(&mut output, &line);
            break;
        }
        output.push_str(&line);
    }
    output
}

fn unaligned_preview(source: &str) -> String {
    let mut output = String::from(
        "[Table exceeds 64 columns or 2,000 rows; unaligned source follows.]\n",
    );
    if output.len() + source.len() > MAX_BYTES {
        append_limited(&mut output, source);
    } else {
        output.push_str(source);
    }
    output
}

fn append_limited(output: &mut String, next: &str) {
    let budget = MAX_BYTES - LIMIT_NOTICE.len();
    output.truncate(prefix(output, budget).len());
    output.push_str(prefix(next, budget - output.len()));
    output.push_str(LIMIT_NOTICE);
}

fn prefix(text: &str, limit: usize) -> &str {
    let mut end = limit.min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

fn cells(source: &str) -> Option<Vec<Vec<String>>> {
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut cell = String::new();
    let mut inside = false;
    for event in Parser::new_ext(source, Options::ENABLE_TABLES) {
        match event {
            Event::Start(Tag::TableHead | Tag::TableRow) => {
                if rows.len() == 2000 {
                    return None;
                }
                rows.push(Vec::new())
            }
            Event::Start(Tag::TableCell) => {
                inside = true;
                cell.clear();
            }
            Event::Text(text) | Event::Code(text) if inside => {
                cell.push_str(&text)
            }
            Event::SoftBreak | Event::HardBreak if inside => cell.push(' '),
            Event::End(TagEnd::TableCell) => {
                if let Some(row) = rows.last_mut() {
                    if row.len() == 64 {
                        return None;
                    }
                    row.push(std::mem::take(&mut cell));
                }
                inside = false;
            }
            _ => {}
        }
    }
    Some(rows)
}
