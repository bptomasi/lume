//! Prefer usable publisher content, then its summary after HTML normalization.
//!
//! An empty content element must not hide a useful summary. Active-only HTML
//! also falls through after sanitization. References remain legitimate feed
//! content and are preserved; no article-page request happens here.

use feed_rs::model::Entry;

use crate::content::{self, BodyKind};

pub(super) fn normalize(
    entry: &Entry,
    address: &str,
) -> (String, crate::model::FeedOrigin) {
    use crate::model::FeedOrigin;
    let content = entry.content.as_ref().and_then(|content| {
        content.body.as_deref().map(|body| {
            (body, content.content_type.to_string() != "text/plain")
        })
    });
    let summary = entry.summary.as_ref().map(|summary| {
        (
            summary.content.as_str(),
            summary.content_type.to_string() != "text/plain",
        )
    });
    [(content, FeedOrigin::Feed), (summary, FeedOrigin::Summary)]
        .into_iter()
        .filter_map(|(candidate, origin)| {
            candidate.map(|(body, html)| {
                (content::article_markdown(body, html, address), origin)
            })
        })
        .find(|(body, _)| content::body_kind(body) != BodyKind::Missing)
        .unwrap_or_else(|| {
            (content::MISSING_BODY.to_owned(), FeedOrigin::Missing)
        })
}
