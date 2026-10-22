//! Nested publisher lists retain markers, hanging layout and source identity.

use anyhow::{Context, Result, ensure};
use lume::{content, reader};

pub(super) fn run(iterations: usize, stress: bool) -> Result<()> {
    // Keep the stress fixture below the normalizer's explicit 256 KiB cap.
    let count = if stress { 768 } else { 128 };
    legacy_cache(iterations, count)?;
    let html = fixture(count);
    let source = content::article_text(&html, true);
    let last = format!("reservation-{}", count - 1);
    println!("list input · items={count} · HTML bytes={}", html.len());
    super::measure(
        "publisher list normalization + hierarchy",
        iterations,
        || {
            let normalized = content::article_text(&html, true);
            ensure!(normalized == source, "List normalization changed");
            ensure!(
                normalized.contains("   - Check reservations"),
                "Nested list lost its indentation"
            );
            Ok(())
        },
    )?;
    super::measure("nested list layout + source identity", iterations, || {
        let rendered = reader::render(&source, 48);
        ensure!(
            rendered.text.matches("• Check reservations").count() == count,
            "List item markers were lost"
        );
        ensure!(rendered.insets.iter().any(|inset| *inset > 0), "No wraps");
        let start = rendered.text.find(&last).context("Final link missing")?;
        let range =
            reader::to_source(&rendered.mappings, start..start + last.len())
                .context("Final source missing")?;
        ensure!(source[range] == last, "List source drifted");
        ensure!(rendered.links.len() == count, "List links changed");
        Ok(())
    })
}

fn legacy_cache(iterations: usize, count: usize) -> Result<()> {
    let mut source = "Cached publisher list:\n\n".to_owned();
    for index in 0..count {
        source.push_str(&format!("\n-\n\nQueue-{index} capacity\n\n"));
    }
    let last = format!("Queue-{}", count - 1);
    super::measure("legacy list layout + original anchors", iterations, || {
        let rendered = reader::render(&source, 48);
        ensure!(
            rendered.text.matches("• Queue-").count() == count,
            "Cached list markers remained detached"
        );
        let start = rendered.text.find(&last).context("Final item missing")?;
        let original = source.find(&last).context("Original item missing")?;
        let range =
            reader::to_source(&rendered.mappings, start..start + last.len())
                .context("Cached item source missing")?;
        ensure!(range == (original..original + last.len()), "Anchor drifted");
        Ok(())
    })
}

fn fixture(count: usize) -> String {
    let mut html = "<ol start='7'>".to_owned();
    for index in 0..count {
        html.push_str(&format!(
            "<li><p>Queue {index}: compare requests with nominal quota, \
             current usage, borrowing limits and admission conditions.</p>\
             <ul><li>Check reservations using \
             <a href='https://example.org/{index}'>reservation-{index}</a>\
             </li><li> </li><li><a id='empty-{index}'></a></li>\
             <li><ul><li>Nested capacity</li></ul></li></ul></li>"
        ));
    }
    html.push_str("</ol>");
    html
}
