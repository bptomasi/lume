//! Feed-decoding and terminal-content contracts with independent fixtures.

use anyhow::Result;
use lume::{content, source};

#[test]
fn rss_prefers_full_content_and_preserves_code() -> Result<()> {
    let feed = source::parse(
        include_bytes!("fixtures/rss.xml"),
        "https://journal.example/feed",
    )?;
    assert_eq!(feed.title, "Systems Journal");
    assert_eq!(feed.articles.len(), 1);
    let article = &feed.articles[0];
    assert_eq!(article.external_id, "article-42");
    assert_eq!(article.title, "Cache & consequence");
    assert_eq!(article.url, "https://journal.example/cache");
    assert_eq!(article.published, 1_792_065_600);
    assert!(article.body.contains("Keep **one** copy & measure."));
    assert!(
        article
            .body
            .contains("let answer = 42;\n    use_it(answer);")
    );
    assert!(!article.body.contains("short summary"));
    Ok(())
}

#[test]
fn atom_uses_alternate_link_and_resolves_relative_addresses() -> Result<()> {
    let feed = source::parse(
        include_bytes!("fixtures/atom.xml"),
        "https://rust.example/feeds/atom",
    )?;
    let article = &feed.articles[0];
    assert_eq!(feed.title, "Rust Notes");
    assert_eq!(article.url, "https://rust.example/ownership");
    assert_eq!(article.author, "Ada");
    assert_eq!(article.body, "A *small* contract.");
    Ok(())
}

#[test]
fn json_plain_text_keeps_angle_brackets() -> Result<()> {
    let feed = source::parse(
        include_bytes!("fixtures/feed.json"),
        "https://notes.example/feed",
    )?;
    assert_eq!(
        feed.articles[0].body,
        "Vec<T> keeps <T> as plain text.\n\nAnd paragraphs."
    );
    Ok(())
}

#[test]
fn invalid_and_oversized_feeds_are_explicit_errors() {
    assert!(source::parse(b"not a feed", "https://example.org").is_err());
    assert!(
        source::parse(&vec![b'x'; 4 * 1024 * 1024 + 1], "https://example.org")
            .is_err()
    );
    assert!(source::feed_url("file:///etc/passwd").is_err());
    assert!(source::feed_url("https://user:secret@example.org/feed").is_err());
    assert!(source::feed_url("/feed.xml").is_err());
}

#[test]
fn articles_without_guids_keep_identity_when_headlines_change() -> Result<()> {
    let fixture = include_str!("fixtures/rss.xml");
    let fixture =
        fixture.replace("<guid isPermaLink=\"false\">article-42</guid>", "");
    let first =
        source::parse(fixture.as_bytes(), "https://journal.example/feed")?;
    let revised =
        fixture.replace("Cache &amp; consequence", "A corrected headline");
    let second =
        source::parse(revised.as_bytes(), "https://journal.example/feed")?;
    assert_eq!(
        first.articles[0].external_id,
        second.articles[0].external_id
    );
    assert_eq!(second.articles[0].title, "A corrected headline");
    Ok(())
}

#[test]
fn markup_cannot_emit_terminal_controls_or_embedded_code() {
    // Exact adversarial wire fragment: the entity decodes to an ESC byte.
    let html = "<p>A&#27;[31m &nbsp; B</p><script>bad()</script><style>bad</style><p>tail</p>";
    let text = content::article_text(html, true);
    assert!(!text.contains('\u{1b}'));
    assert!(!text.contains("bad"));
    assert!(text.contains("tail"));
    assert_eq!(content::label("hello\n\u{1b}world", 8), "hello  w");
    assert_eq!(
        content::article_text("<p>valid</p><unclosed", true),
        "valid"
    );
}

#[test]
fn rss_empty_or_sanitized_content_falls_back_without_losing_references()
-> Result<()> {
    let parsed = source::parse(
        include_bytes!("fixtures/partial-content.xml"),
        "https://journal.example/feed",
    )?;
    let bodies = parsed
        .articles
        .iter()
        .map(|article| (article.external_id.as_str(), article.body.as_str()))
        .collect::<Vec<_>>();
    assert_eq!(
        bodies,
        vec![
            ("empty", "A useful short summary."),
            ("whitespace", "A **formatted** summary."),
            ("active-only", "Safe fallback text."),
            (
                "discussion-only",
                "[Comments](<https://community.example/discussion>)"
            ),
            ("missing", content::MISSING_BODY),
        ]
    );
    assert_eq!(parsed.articles[3].url, "https://journal.example/article");
    Ok(())
}

#[test]
fn atom_empty_inline_and_external_content_use_the_publisher_summary()
-> Result<()> {
    let parsed = source::parse(
        include_bytes!("fixtures/partial-content.atom"),
        "https://journal.example/feed",
    )?;
    assert_eq!(parsed.articles.len(), 2);
    assert_eq!(parsed.articles[0].body, "A *useful* summary.");
    assert_eq!(parsed.articles[1].body, "Read the summary offline.");
    Ok(())
}

#[test]
fn json_empty_html_or_text_does_not_hide_readable_alternatives() -> Result<()> {
    let parsed = source::parse(
        include_bytes!("fixtures/partial-content.json"),
        "https://journal.example/feed",
    )?;
    let bodies = parsed
        .articles
        .iter()
        .map(|article| article.body.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        bodies,
        vec![
            "A useful plain summary.",
            "Vec<T> remains literal.",
            "A second plain summary.",
        ]
    );
    Ok(())
}

#[test]
fn article_links_keep_fragments_while_subscription_identity_discards_them()
-> Result<()> {
    let fixture = include_str!("fixtures/rss.xml").replace(
        "https://journal.example/cache",
        "https://journal.example/cache#implementation",
    );
    let feed =
        source::parse(fixture.as_bytes(), "https://journal.example/feed")?;

    assert_eq!(
        feed.articles[0].url,
        "https://journal.example/cache#implementation"
    );
    assert_eq!(
        source::canonical_article_url(&feed.articles[0].url),
        "https://journal.example/cache"
    );
    assert_eq!(
        source::feed_url("https://journal.example/feed#section")?.as_str(),
        "https://journal.example/feed"
    );
    Ok(())
}
