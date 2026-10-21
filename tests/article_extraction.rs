//! Full-page extraction preserves technical material and rejects bad inputs.

use anyhow::Result;
use lume::{reader, source::article};

#[test]
fn extraction_keeps_main_text_code_tables_and_relative_link_targets()
-> Result<()> {
    let document = article::extract(
        include_str!("fixtures/article.html"),
        "https://journal.example/posts/queues/",
    )?;
    assert!(document.body.contains("## Preserve the contract"));
    assert!(
        document.body.contains(
            "fn admit<T>(item: T) -> Result<T, T> {\n    Ok(item)\n}"
        )
    );
    assert!(document.body.contains("~~~rust\n"));
    assert!(document.body.contains(
        "~~~sql\nSELECT body\nFROM article_documents\nWHERE article_id = 42;"
    ));
    assert!(document.body.contains("Bounded admission"));
    assert!(document.body.contains("Preserve on failure"));
    assert!(!document.body.contains("Unrelated navigation marker"));
    assert!(!document.body.contains("Unrelated sidebar marker"));
    assert!(!document.body.contains("invisibleTrackingMarker"));
    let rendered = reader::render(&document.body, 72);
    assert!(rendered.links.iter().any(|(title, url)| {
        title == "recovery reference"
            && url == "https://journal.example/posts/reference#recovery"
    }));
    assert!(rendered.code.iter().any(|code| *code));
    Ok(())
}

#[test]
fn empty_active_only_invalid_and_oversized_pages_are_explicit_failures() {
    let url = "https://journal.example/article";
    assert!(article::extract("", url).is_err());
    assert!(article::extract("<script>hidden()</script>", url).is_err());
    assert!(
        article::extract("<p>A useful paragraph.</p>", "file:///tmp/page")
            .is_err()
    );
    assert!(article::extract(&"x".repeat(2 * 1024 * 1024 + 1), url).is_err());
}

#[test]
fn element_limit_rejects_large_documents_without_returning_partial_success() {
    let html = format!(
        "<html><body>{}</body></html>",
        "<div>node</div>".repeat(20_001)
    );
    let error = article::extract(&html, "https://journal.example/article");
    assert!(error.is_err());
}

#[test]
fn copyright_only_results_fail_but_short_articles_and_legal_topics_survive()
-> Result<()> {
    let footer = "Copyright © 2026, Eklektix, Inc. Comments and public \
        postings are copyrighted by their creators. Linux is a registered \
        trademark of Linus Torvalds";
    let html = format!("<article><p>{footer}</p></article>");

    let result = article::extract_with_selector(
        &html,
        "https://journal.example/article",
        "article",
    );

    assert!(result.is_err());
    assert!(article::rules::rejection(footer).is_some());
    for text in [
        "A useful short note.",
        "Copyright law affects how we distribute software.",
        "Actual technical reasoning.\n\nCopyright © 2026. All rights reserved.",
    ] {
        assert!(article::rules::rejection(text).is_none());
        article::extract_with_selector(
            &format!("<article><p>{text}</p></article>"),
            "https://journal.example/article",
            "article",
        )?;
    }
    Ok(())
}
