//! Publisher-source identity, cache validation and feed fallback.
use super::ReaderState;
use crate::app::reading_data::ReadingData;
use crate::{
    content::{self, BodyKind},
    model::ContentSource,
};

impl ReaderState {
    pub(super) fn merge_article_links(
        &self,
        mut links: Vec<(String, String)>,
    ) -> Vec<(String, String)> {
        if self.publisher.source != ContentSource::Full {
            return links;
        }
        // Preserve encounter order while avoiding quadratic URL comparisons
        // when a community feed contains many references.
        let mut seen = links
            .iter()
            .map(|(_, url)| url.clone())
            .collect::<std::collections::BTreeSet<_>>();
        for link in &self.publisher.feed_links {
            if seen.insert(link.1.clone()) {
                links.push(link.clone());
            }
        }
        links
    }

    pub(super) fn apply_article_content(
        &mut self,
        data: &mut ReadingData,
        preferred: ContentSource,
    ) {
        self.publisher.body = std::mem::take(&mut data.body);
        self.publisher.origin = data.origin;
        self.publisher.feed_kind = content::body_kind(&self.publisher.body);
        self.publisher.feed_links.clear();
        self.publisher.cache = data.document.take();
        self.publisher.source = ContentSource::Feed;
        self.publisher.revision = 0;
        self.publisher.fetch_policy = data.policy;
        if preferred == ContentSource::Full
            && let Some(document) = &self.publisher.cache
            && self
                .publisher
                .article
                .as_ref()
                .is_some_and(|article| article.url == document.url)
            && !document.body.is_empty()
            && crate::content::extraction::rejection(&document.body).is_none()
        {
            if self.publisher.feed_kind == BodyKind::ReferencesOnly {
                self.publisher.feed_links =
                    crate::reader::render_with_plain_links(
                        &self.publisher.body,
                    );
            }
            self.publisher.body.clone_from(&document.body);
            self.publisher.source = ContentSource::Full;
            self.publisher.revision = document.revision;
        }
    }
}
