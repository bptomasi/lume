//! Link labels retain their own text across wraps, formatting and image slots.

use lume::reader;

#[test]
fn link_labels_span_hard_breaks_without_including_preceding_text() {
    let body = "Long unrelated prefix [alpha  \nbeta **bold** `code`](https://example.org/a).";

    let rendered = reader::render(body, 10);

    assert_eq!(
        rendered.links,
        [(
            "alpha beta bold code".into(),
            "https://example.org/a".into()
        )]
    );
}

#[test]
fn linked_images_keep_the_alternative_text_as_the_link_label() {
    let body = "Prefix [![A useful diagram](https://example.org/a.png)](https://example.org/a).";

    let rendered = reader::render(body, 80);

    assert_eq!(
        rendered.links,
        [("A useful diagram".into(), "https://example.org/a".into())]
    );
    assert_eq!(rendered.images.len(), 1);
}

#[test]
fn link_order_is_first_seen_and_duplicate_labels_use_the_last_occurrence() {
    let body = "[first](https://example.org/a) [other](https://example.org/b) [last](https://example.org/a)";
    let rendered = reader::render(body, 20);
    assert_eq!(
        rendered.links,
        [
            ("last".into(), "https://example.org/a".into()),
            ("other".into(), "https://example.org/b".into()),
        ]
    );
}

#[test]
fn labels_are_bounded_by_characters_without_changing_visible_text() {
    let label = "界".repeat(800);
    let body = format!("[{label}](https://example.org/a)");

    let rendered = reader::render(&body, 80);

    assert_eq!(rendered.links[0].0, "界".repeat(500));
    assert!(rendered.text.contains(&label));
}

#[test]
fn unsupported_destinations_never_enter_or_relabel_the_reference_list() {
    let body = "[safe](https://example.org/a) [blocked](javascript:alert) [local](file:///etc/passwd)";

    let rendered = reader::render(body, 80);

    assert_eq!(
        rendered.links,
        [("safe".into(), "https://example.org/a".into())]
    );
    assert!(rendered.text.contains("blocked"));
    assert!(rendered.text.contains("local"));
}

#[test]
fn explicit_html_ids_and_duplicate_heading_slugs_remain_invisible_and_navigable()
 {
    let html = "<p><a href='#a%2Bb%26c'>Jump</a></p>\
        <h2 id='a+b&amp;c'>Some heading</h2><p>Target text.</p>\
        <h2>Repeat</h2><p>First.</p><h2>Repeat</h2><p>Second.</p>";
    let markdown = lume::content::article_markdown(html, true, "");

    let index = reader::index::DocumentIndex::parse(&markdown);
    let rendered = reader::render(&markdown, 20);

    assert!(index.navigation.anchor("a%2Bb%26c").is_some());
    assert!(index.navigation.anchor("some-heading").is_some());
    assert!(index.navigation.anchor("repeat-1").is_some());
    assert!(!rendered.text.contains("<a"));
    assert!(!rendered.text.contains("</a>"));
    assert_eq!(rendered.links.len(), 0);
    assert_eq!(
        index
            .navigation
            .link_at(1)
            .map(|link| link.destination.as_str()),
        Some("#a%2Bb%26c")
    );
}
