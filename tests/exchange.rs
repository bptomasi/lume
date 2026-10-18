//! Discovery, OPML and Markdown exchange failure contracts.

mod common;

use anyhow::Result;
use lume::{
    exchange::{markdown, opml},
    source::discovery,
    store::Store,
};

#[test]
fn html5_discovery_resolves_first_base_ignores_scripts_and_deduplicates()
-> Result<()> {
    let html = r#"
        <base href="/publications/"><base href="https://wrong.example/">
        <SCRIPT>const fake = '<link rel=alternate type=application/rss+xml href="/fake">';</SCRIPT>
        <LINK REL="alternate other" TYPE="application/rss+xml"
            HREF="rss?lang=en&amp;tag=rust" TITLE="Rust &amp; systems">
        <link rel=alternate type=application/rss+xml href="rss?lang=en&amp;tag=rust">
        <link rel=alternate type=application/atom+xml href="javascript:bad()">
        <link rel=alternate type=application/feed+json href="https://json.example/feed">
    "#;

    let feeds = discovery::links(html, "https://journal.example/start")?;

    assert_eq!(feeds.len(), 2);
    assert_eq!(
        feeds[0].url,
        "https://journal.example/publications/rss?lang=en&tag=rust"
    );
    assert_eq!(feeds[0].title, "Rust & systems");
    Ok(())
}

#[test]
fn nested_opml_import_is_idempotent_and_export_round_trips() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let store = Store::open(&directory.path().join("import.sqlite3"))?;
    let candidates = opml::decode(OPML)?;
    assert_eq!(candidates.len(), 2);

    assert_eq!(store.import_feeds(&candidates)?, 2);
    assert_eq!(store.import_feeds(&candidates)?, 0);
    let target = directory.path().join("subscriptions.opml");
    opml::export(&store, &target)?;

    assert_eq!(store.feeds()?.len(), 2);
    assert_eq!(opml::decode(&std::fs::read_to_string(target)?)?.len(), 2);
    Ok(())
}

#[test]
fn invalid_import_and_excessive_nesting_cannot_change_subscriptions()
-> Result<()> {
    let (_directory, store, _feed) = common::seeded_store()?;
    let invalid =
        OPML.replace("https://other.example/rss", "file:///etc/passwd");
    assert!(opml::decode(&invalid).is_err());
    let nested = format!(
        "<opml><body>{}{}</body></opml>",
        "<outline>".repeat(65),
        "</outline>".repeat(65)
    );
    assert!(opml::decode(&nested).is_err());
    assert_eq!(store.feeds()?.len(), 1);
    Ok(())
}

#[test]
fn markdown_preserves_source_quote_and_note_and_never_overwrites() -> Result<()>
{
    let (directory, store, _feed) = common::seeded_store()?;
    let id = store.library()?.articles[0].id;
    store.save_highlight(
        id,
        "Keep one copy",
        0,
        "My independent observation",
    )?;
    let target = directory.path().join("notebook.md");

    markdown::notebook(&store, &target)?;
    let expected = std::fs::read_to_string(&target)?;

    assert!(expected.contains("https://journal.example/cache"));
    assert!(expected.contains("> Keep one copy"));
    assert!(expected.contains("My independent observation"));
    assert!(markdown::article(&store, id, &target).is_err());
    assert_eq!(std::fs::read_to_string(target)?, expected);
    Ok(())
}

#[test]
fn rich_reader_keeps_code_columns_links_and_terminal_safe_html() {
    let body = lume::content::article_markdown(
        "<h2>Ownership</h2><p>Read <a href='/more#evidence'>the evidence</a>.</p><pre><code>  let n = 42;\n</code></pre><script>bad()</script>",
        true,
        "https://journal.example/article",
    );
    let rendered = lume::reader::render(&body, 18);
    let text = rendered
        .lines
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    assert!(text.iter().any(|line| line == "Ownership"));
    assert!(text.iter().any(|line| line == "  let n = 42;"));
    assert_eq!(rendered.links[0].0, "the evidence");
    assert_eq!(rendered.links[0].1, "https://journal.example/more#evidence");
    assert_eq!(rendered.lines.len(), rendered.ranges.len());
    assert!(!text.join("\n").contains("bad()"));
}

const OPML: &str = r#"
<opml version="2.0">
    <body>
        <outline text="Computing">
            <outline text="Rust &amp; Systems" xmlUrl="https://journal.example/rss"/>
            <outline text="Duplicate" xmlUrl="https://journal.example/rss"/>
            <outline text="Another source" xmlUrl="https://other.example/rss"/>
        </outline>
    </body>
</opml>
"#;

#[test]
fn unicode_quotes_and_indented_code_keep_valid_original_source_ranges() {
    let body = "> 🦀 café — thoughtful reading and independent notes.\n\
                >\n\
                > ~~~~rust\n\
                > let café = \"🦀\";\n\
                > let evidence = \"ação\";\n\
                > ~~~~\n";
    for width in [8, 12, 30, 82] {
        let rendered = lume::reader::render(body, width);
        for range in rendered.ranges {
            assert!(body.get(range.clone()).is_some(), "{range:?}");
        }
    }
}
