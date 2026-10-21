//! Article source selection, background request policy and deferred completion.
//!
//! Requests capture IDs/URLs. Completion never retargets a draft or selection;
//! a safe source switch waits until normal mode without a visual selection.

use anyhow::{Context, Result, bail, ensure};

use crate::{
    content::{self, BodyKind},
    model::{ArticleTarget, ContentSource, FetchPolicy},
};

use super::{Action, App, Focus, Mode, View};

impl App {
    pub(super) fn merge_article_links(
        &self,
        mut links: Vec<(String, String)>,
    ) -> Vec<(String, String)> {
        if self.content_source != ContentSource::Full {
            return links;
        }
        // Preserve encounter order while avoiding quadratic URL comparisons
        // when a community feed contains many references.
        let mut seen = links
            .iter()
            .map(|(_, url)| url.clone())
            .collect::<std::collections::BTreeSet<_>>();
        for link in &self.feed_links {
            if seen.insert(link.1.clone()) {
                links.push(link.clone());
            }
        }
        links
    }

    pub(super) fn apply_article_content(
        &mut self,
        data: &mut super::reading_data::ReadingData,
    ) {
        self.body = std::mem::take(&mut data.body);
        self.feed_origin = data.origin;
        self.feed_body_kind = content::body_kind(&self.body);
        self.feed_links.clear();
        self.document = data.document.take();
        self.content_source = ContentSource::Feed;
        self.content_revision = 0;
        self.fetch_policy = data.policy;
        if self.workspace.content_source == ContentSource::Full
            && let Some(document) = &self.document
            && self
                .current()
                .is_some_and(|article| article.url == document.url)
            && !document.body.is_empty()
            && crate::source::article::rules::rejection(&document.body)
                .is_none()
        {
            if self.feed_body_kind == BodyKind::ReferencesOnly {
                self.feed_links =
                    crate::reader::render_with_plain_links(&self.body);
            }
            self.body.clone_from(&document.body);
            self.content_source = ContentSource::Full;
            self.content_revision = document.revision;
        }
    }

    pub(super) fn selected_article_target(&self) -> Result<ArticleTarget> {
        ensure!(
            self.view != View::Trash,
            "Restore the article before fetching"
        );
        let article = self.current().context("Select an article first")?;
        let url = crate::source::link_url(&article.url)
            .context("This entry has no usable original article URL")?;
        Ok(ArticleTarget {
            article: article.id,
            url: url.to_string(),
        })
    }

    /// Choose a request only for the opened reader, respecting retry cooldown.
    /// Network availability and worker admission belong to the terminal runtime.
    pub fn automatic_article_request(&self, now: i64) -> Option<ArticleTarget> {
        if self.demo
            || self.navigation_loading
            || self.focus != Focus::Reader
            || self.workspace.content_source == ContentSource::Feed
            || self.view == View::Annotated
            || self.article_fetching.is_some()
            || !matches!(self.mode, Mode::Normal)
            || self.selection.is_some()
        {
            return None;
        }
        let target = self.selected_article_target().ok()?;
        if self
            .last_article_attempt
            .as_ref()
            .is_some_and(|(previous, at)| {
                previous == &target && now.saturating_sub(*at) < 900
            })
        {
            return None;
        }
        if let Some(cache) = &self.document {
            if cache.url == target.url && !cache.body.is_empty() {
                return None;
            }
            if cache.attempt_url == target.url
                && now.saturating_sub(cache.attempted_at) < 900
            {
                return None;
            }
        }
        match self.fetch_policy {
            FetchPolicy::Manual => None,
            FetchPolicy::Missing if self.feed_body_kind == BodyKind::Text => {
                None
            }
            _ => Some(target),
        }
    }

    /// Reflect an admitted request without changing its selected article.
    pub fn article_fetch_started(&mut self, target: ArticleTarget) {
        self.workspace.content_source = ContentSource::Full;
        self.last_article_attempt =
            Some((target.clone(), chrono::Utc::now().timestamp()));
        self.article_fetching = Some(target);
        self.status = "Fetching article… · reading remains available".into();
    }

    /// Stage completion; reading/draft changes wait for a safe UI boundary.
    pub fn article_fetch_finished(
        &mut self,
        target: ArticleTarget,
        error: Option<&str>,
    ) {
        if self.article_fetching.as_ref() == Some(&target) {
            self.article_fetching = None;
        }
        self.pending_document = Some(target);
        self.status = error.map_or_else(
            || "Full article cached · :content switches sources".into(),
            |message| {
                format!(
                    "Article fetch: {} · :fetch retries",
                    content::label(message, 2000)
                )
            },
        );
    }

    /// Apply a captured completion only when no draft or selection is active.
    ///
    /// # Errors
    /// Returns local storage errors; completed content remains cached on disk.
    pub fn apply_document_completion(&mut self) -> Result<bool> {
        let Some(target) = &self.pending_document else {
            return Ok(false);
        };
        if !matches!(self.mode, Mode::Normal) || self.selection.is_some() {
            return Ok(false);
        }
        let selected = self.current().is_some_and(|article| {
            article.id == target.article && article.url == target.url
        });
        self.pending_document = None;
        if selected {
            let cursor = self.cursor_offset();
            let previous_body = self.body.clone();
            let previous_source = self.content_source;
            self.save_reading_position()?;
            self.load_current()?;
            if self.body == previous_body
                && self.content_source == previous_source
            {
                self.pending_anchor = None;
                self.place_cursor(cursor);
                self.ensure_cursor_visible();
            }
        }
        Ok(selected)
    }

    pub(super) fn document_command(
        &mut self,
        verb: &str,
        argument: &str,
    ) -> Result<Option<Action>> {
        let action = match verb {
            "fetch" => {
                ensure!(argument.is_empty(), "Use :fetch without arguments");
                ensure!(!self.demo, "Article fetching needs a real library");
                ensure!(
                    self.article_fetching.is_none(),
                    "An article fetch is already running"
                );
                Action::FetchArticle(self.selected_article_target()?)
            }
            "content" => return self.select_content(argument).map(Some),
            "fetch-policy" => {
                self.change_fetch_policy(argument)?;
                Action::None
            }
            _ => return Ok(None),
        };
        Ok(Some(action))
    }

    fn select_content(&mut self, argument: &str) -> Result<Action> {
        let source = match argument {
            "" if self.content_source == ContentSource::Feed => {
                ContentSource::Full
            }
            "" | "feed" => ContentSource::Feed,
            "full" => ContentSource::Full,
            _ => bail!("Use :content feed or :content full"),
        };
        self.save_reading_position()?;
        self.workspace.content_source = source;
        self.load_current()?;
        self.status = match self.content_source {
            ContentSource::Full => {
                "Full article cached · :content feed returns to the feed".into()
            }
            ContentSource::Feed => {
                "Feed content · :fetch retrieves the original article".into()
            }
        };
        if source == ContentSource::Full && self.content_source != source {
            return self
                .document_command("fetch", "")?
                .context("Missing article fetch command");
        }
        Ok(Action::None)
    }

    fn change_fetch_policy(&mut self, argument: &str) -> Result<()> {
        let feed = if let View::Feed(id) = self.view {
            id
        } else {
            self.current()
                .context("Select an article or subscription")?
                .feed_id
        };
        let policy = match argument {
            "missing" => FetchPolicy::Missing,
            "always" => FetchPolicy::Always,
            "manual" => FetchPolicy::Manual,
            _ => bail!("Use :fetch-policy missing, always or manual"),
        };
        self.store.set_fetch_policy(feed, policy)?;
        self.fetch_policy = policy;
        self.status = format!("Source fetch policy: {}", policy.as_str());
        Ok(())
    }
}
