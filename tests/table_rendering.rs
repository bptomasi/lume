//! Native table layout, source correspondence and bounded fallback contracts.

use anyhow::{Context, Result};
use lume::{content, reader};
use ratatui::style::{Color, Modifier};

const TABLE: &str = "| Item | Count | State |\n\
    | :--- | ---: | :---: |\n\
    | **Rust** | 9 | yes |\n\
    | 界 | 100 | *ok* |\n";

#[test]
fn reader_draws_rounded_borders_and_honors_unicode_and_alignment() {
    let rendered = reader::render(TABLE, 80);
    let actual = rendered
        .lines
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    assert_eq!(
        actual,
        [
            "╭──────┬───────┬───────╮",
            "│ Item │ Count │ State │",
            "├──────┼───────┼───────┤",
            "│ Rust │     9 │  yes  │",
            "│ 界   │   100 │  ok   │",
            "╰──────┴───────┴───────╯",
            "",
        ]
    );
    assert_eq!(rendered.table_rows.len(), 1);
    assert_eq!(rendered.table_rows[0], 0..6);
    assert!(rendered.code[..6].iter().all(|line| *line));
    assert!(rendered.lines[..6].iter().all(|line| line.width() == 24));
    let header = &rendered.lines[1];
    assert!(header.spans.iter().any(|span| {
        span.content == "Item"
            && span.style.fg == Some(Color::Rgb(225, 179, 109))
            && span.style.add_modifier.contains(Modifier::BOLD)
    }));
}

#[test]
fn table_cells_retain_exact_source_anchors_and_inline_styles() -> Result<()> {
    let rendered = reader::render(TABLE, 12);
    let start = rendered.text.find("Rust").context("Missing cell")?;
    let source = reader::to_source(&rendered.mappings, start..start + 4)
        .context("Missing source mapping")?;
    assert_eq!(&TABLE[source.clone()], "Rust");
    let projected = reader::to_display(&rendered.mappings, source);
    assert_eq!(projected.len(), 1);
    assert_eq!(projected[0], start..start + 4);
    assert!(reader::to_source(&rendered.mappings, 0..3).is_none());
    assert_eq!(reader::render(TABLE, 120).text, rendered.text);
    let row = &rendered.lines[4];
    assert!(row.spans.iter().any(|span| {
        span.content == "ok"
            && span.style.add_modifier.contains(Modifier::ITALIC)
    }));
    Ok(())
}

#[test]
fn links_entities_escaped_pipes_and_inline_code_survive_cell_layout()
-> Result<()> {
    let source = "| Key | Value |\n| --- | --- |\n\
        | a\\|b | [Reference](https://journal.example/ref) |\n\
        | `Vec<T>` | &amp; ![diagram](https://journal.example/image.png) |\n";
    let rendered = reader::render(source, 20);
    assert!(rendered.text.contains("a|b"));
    assert!(rendered.text.contains("Vec<T>"));
    assert!(rendered.text.contains("& diagram"));
    assert!(
        rendered.images.is_empty(),
        "Cell images must not break grid rows"
    );
    assert_eq!(
        rendered.links,
        [("Reference".into(), "https://journal.example/ref".into())]
    );
    for (visible, original) in
        [("a|b", "a\\|b"), ("Vec<T>", "Vec<T>"), ("&", "&amp;")]
    {
        let start = rendered
            .text
            .find(visible)
            .context("Missing visible text")?;
        let source_range =
            reader::to_source(&rendered.mappings, start..start + visible.len())
                .context("Missing cell source")?;
        assert_eq!(&source[source_range], original);
    }
    Ok(())
}

#[test]
fn html_tables_share_the_reader_grid_and_keep_empty_cells() {
    let html = "<table><tr><th>Name</th><th>Value</th></tr>\
        <tr><td>é</td><td></td></tr>\
        <tr><td>e&#769;</td><td>界</td></tr></table>";
    let source =
        content::article_markdown(html, true, "https://journal.example");
    let rendered = reader::render(&source, 20);
    assert!(rendered.text.contains("│ é    │       │"));
    assert!(rendered.text.contains("│ é    │ 界    │"));
    assert!(rendered.lines[..6].iter().all(|line| line.width() == 16));
    assert_eq!(reader::table_view::render(&source), rendered.text);
}

#[test]
fn pathological_tables_fall_back_without_hiding_source_or_later_paragraphs() {
    let wide = format!(
        "| Header |\n| --- |\n| {} |\n\nAfter table.",
        "x".repeat(5000)
    );
    let tall = format!(
        "| Header |\n| --- |\n{}\nAfter table.",
        "| retained |\n".repeat(2000)
    );
    for source in [wide, tall] {
        let rendered = reader::render(&source, 80);
        assert!(rendered.table_rows.is_empty());
        assert!(rendered.text.contains("original Markdown follows"));
        assert!(rendered.text.contains("After table."));
        assert!(rendered.text.contains("| Header |"));
    }
}

#[test]
fn padding_is_bounded_across_all_tables_in_an_article() {
    let header = vec!["a".repeat(96); 8].join(" | ");
    let separators = ["---"; 8].join(" | ");
    let rows = format!("| {} |\n", [""; 8].join(" | ")).repeat(800);
    let table = format!("| {header} |\n| {separators} |\n{rows}");
    let source = format!("{table}\n{table}\nAfter everything.");

    let rendered = reader::render(&source, 80);

    assert_eq!(rendered.table_rows.len(), 1);
    assert!(rendered.text.contains("original Markdown follows"));
    assert!(rendered.text.contains("After everything."));
    assert!(rendered.text.len() < 1024 * 1024);
}

#[test]
fn fully_bold_publisher_cells_become_the_header_without_a_blank_row() {
    let html = "<table><tbody><tr>\
        <td><strong>Same model</strong></td>\
        <td><strong>Shell + kubectl</strong></td>\
        <td><strong>Read-only context</strong></td></tr>\
        <tr><td>Correct</td><td>45</td><td>46</td></tr>\
        </tbody></table>";

    let source = content::article_markdown(html, true, "https://example.org");
    let rendered = reader::render(&source, 80);

    assert_eq!(rendered.table_rows.len(), 1);
    assert_eq!(rendered.table_rows[0], 0..5);
    assert!(rendered.lines[1].to_string().contains("Shell + kubectl"));
    assert!(rendered.lines[3].to_string().contains("Correct"));
    assert!(rendered.lines[1].spans.iter().any(|span| {
        span.content == "Read-only context"
            && span.style.add_modifier.contains(Modifier::BOLD)
    }));
}

#[test]
fn plain_or_partially_bold_first_rows_remain_data() {
    for first in [
        "<td>Value</td><td>42</td>",
        "<td><strong>Value</strong></td><td>42</td>",
        "<td><strong>Value</strong> suffix</td><td><b>42</b></td>",
    ] {
        let html = format!("<table><tr>{first}</tr></table>");
        let source = content::article_markdown(&html, true, "");
        let rendered = reader::render(&source, 80);

        assert!(!rendered.lines[1].to_string().contains("Value"));
        assert!(rendered.lines[3].to_string().contains("Value"));
    }
}
