//! Quote-heavy semantic content survives a misleading legal-footer candidate.

use anyhow::Result;

#[test]
fn readability_keeps_responsive_images_for_high_resolution_selection()
-> Result<()> {
    let html = include_str!("../../../tests/fixtures/article.html")
        .replace("</article>", "<img src='tiny.png' srcset='preview.png 320w, sharp.png 1920w' alt='Diagram'></article>");
    let document = super::extract(&html, "https://journal.example/report")?;
    let rendered = crate::reader::render(&document.body, 80);
    assert_eq!(rendered.images.len(), 1);
    assert_eq!(rendered.images[0].url, "https://journal.example/sharp.png");
    Ok(())
}

#[test]
fn semantic_main_recovers_a_quote_dominated_article() -> Result<()> {
    let html =
        include_str!("../../../tests/fixtures/extraction/quote-main.html");
    let address = "https://journal.example/report";
    let fragment = super::readable_html(html, address)?;
    let candidate = crate::content::article_markdown(&fragment, true, address);
    assert!(
        super::rules::rejection(&candidate).is_some(),
        "Fixture no longer exercises fallback: {candidate}"
    );

    let document = super::extract(html, address)?;

    assert!(document.body.contains("Request cleanup must inspect"));
    assert!(document.body.contains("allocation injection"));
    assert!(!document.body.contains("Copyright ©"));
    Ok(())
}
