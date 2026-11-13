//! Captured article-worker identities and delayed presentation boundaries.

use crate::model::{ArticleTarget, RulePreview};

#[derive(Default)]
pub(super) struct ArticleRequests {
    fetching: Option<ArticleTarget>,
    pending_document: Option<ArticleTarget>,
    pending_rule: Option<RulePreview>,
    last_attempt: Option<(ArticleTarget, i64)>,
}

impl ArticleRequests {
    pub(super) fn fetching(&self) -> Option<&ArticleTarget> {
        self.fetching.as_ref()
    }

    pub(super) fn attempted_recently(
        &self,
        target: &ArticleTarget,
        now: i64,
    ) -> bool {
        self.last_attempt.as_ref().is_some_and(|(previous, at)| {
            previous == target && now.saturating_sub(*at) < 900
        })
    }

    pub(super) fn start_fetch(&mut self, target: ArticleTarget, now: i64) {
        self.last_attempt = Some((target.clone(), now));
        self.fetching = Some(target);
    }

    pub(super) fn start_preview(&mut self, target: ArticleTarget) {
        self.fetching = Some(target);
    }

    pub(super) fn finish_fetch(&mut self, target: ArticleTarget) -> bool {
        if !self.accept_completion(&target) {
            return false;
        }
        self.pending_document = Some(target);
        true
    }

    pub(super) fn finish_preview(&mut self, preview: RulePreview) -> bool {
        if !self.accept_completion(&preview.target) {
            return false;
        }
        self.pending_rule = Some(preview);
        true
    }

    /// Direct publication without a live request remains supported. A delayed
    /// completion cannot release or replace a different captured active target.
    fn accept_completion(&mut self, target: &ArticleTarget) -> bool {
        if self
            .fetching
            .as_ref()
            .is_some_and(|active| active != target)
        {
            return false;
        }
        self.fetching = None;
        true
    }

    pub(super) fn pending_document(&self) -> Option<&ArticleTarget> {
        self.pending_document.as_ref()
    }

    pub(super) fn acknowledge_document(&mut self) {
        self.pending_document = None;
    }

    pub(super) fn take_preview(&mut self) -> Option<RulePreview> {
        self.pending_rule.take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ExtractedDocument;

    fn target(url: &str) -> ArticleTarget {
        ArticleTarget {
            article: 9,
            url: url.into(),
        }
    }

    fn preview(target: ArticleTarget) -> RulePreview {
        RulePreview {
            target,
            feed: 4,
            previous: String::new(),
            selector: "main".into(),
            document: ExtractedDocument {
                final_url: "https://journal.example/full".into(),
                body: "Reviewed content".into(),
            },
        }
    }

    #[test]
    fn stale_completion_cannot_release_a_different_captured_url() {
        let old = target("https://journal.example/old");
        let active = target("https://journal.example/new");
        let mut requests = ArticleRequests::default();
        requests.start_fetch(active.clone(), 100);

        assert!(!requests.finish_fetch(old.clone()));
        assert!(!requests.finish_preview(preview(old)));
        assert_eq!(requests.fetching(), Some(&active));
        assert!(requests.pending_document().is_none());
        assert!(requests.take_preview().is_none());
        assert!(requests.finish_fetch(active.clone()));
        assert!(requests.fetching().is_none());
        assert_eq!(requests.pending_document(), Some(&active));
    }

    #[test]
    fn preview_preserves_retry_memory_and_supports_direct_publication() {
        let target = target("https://journal.example/article");
        let mut requests = ArticleRequests::default();
        requests.start_fetch(target.clone(), 100);
        assert!(requests.finish_fetch(target.clone()));
        requests.start_preview(target.clone());
        assert!(requests.finish_preview(preview(target.clone())));
        assert!(requests.attempted_recently(&target, 999));
        assert!(!requests.attempted_recently(&target, 1000));
        assert!(requests.take_preview().is_some());

        assert!(requests.finish_preview(preview(target)));
        assert!(requests.take_preview().is_some());
        assert!(requests.take_preview().is_none());
    }
}
