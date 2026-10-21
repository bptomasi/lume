//! Quote-heavy semantic content survives a misleading legal-footer candidate.

use anyhow::Result;

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
