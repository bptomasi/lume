//! Independent network lanes and UI-safe delivery of completed background work.

use std::path::PathBuf;

use anyhow::Result;
use lume::{
    app::{App, Mode},
    model::ArticleTarget,
    source::{Progress, Worker, article},
};

pub(super) struct Background {
    feeds: Option<Worker>,
    articles: Option<article::Worker>,
    pending_feed: Option<Progress>,
}

impl Background {
    pub(super) fn start(path: PathBuf, enabled: bool) -> Result<Self> {
        Ok(Self {
            feeds: enabled.then(|| Worker::start(path.clone())).transpose()?,
            articles: enabled
                .then(|| article::Worker::start(path))
                .transpose()?,
            pending_feed: None,
        })
    }

    pub(super) fn refresh(&self, app: &mut App) {
        if app.refreshing {
            app.status = "A collection pass is already running".into();
            return;
        }
        let Some(worker) = &self.feeds else {
            unavailable(app);
            return;
        };
        match worker.refresh() {
            Ok(()) => {
                app.refreshing = true;
                app.status =
                    "Collecting subscriptions in the background".into();
            }
            Err(error) => app.status = error.to_string(),
        }
    }

    pub(super) fn discover(&self, app: &mut App, url: String) {
        if app.refreshing {
            app.status = "Wait for the active feed request".into();
            return;
        }
        let Some(worker) = &self.feeds else {
            unavailable(app);
            return;
        };
        match worker.discover(url) {
            Ok(()) => {
                app.refreshing = true;
                app.status = "Discovering advertised feeds".into();
            }
            Err(error) => app.status = error.to_string(),
        }
    }

    pub(super) fn fetch(&self, app: &mut App, target: ArticleTarget) {
        let Some(worker) = &self.articles else {
            unavailable(app);
            return;
        };
        match worker.fetch(target.clone()) {
            Ok(()) => app.article_fetch_started(target),
            Err(error) => app.status = error.to_string(),
        }
    }

    pub(super) fn tick(&mut self, app: &mut App) -> Result<bool> {
        let mut dirty = self.receive_feed(app)?;
        if let Some(completion) =
            self.articles.as_ref().and_then(article::Worker::poll)
        {
            if let Some(preview) = completion.preview {
                app.rule_preview_finished(preview);
            } else {
                app.article_fetch_finished(
                    completion.target,
                    completion.error.as_deref(),
                );
            }
            dirty = true;
        }
        dirty |= app.apply_document_completion()?;
        dirty |= app.present_rule_preview();
        if self.articles.is_some()
            && let Some(target) =
                app.automatic_article_request(chrono::Utc::now().timestamp())
        {
            self.fetch(app, target);
            dirty = true;
        }
        Ok(dirty)
    }

    pub(super) fn preview_rule(
        &self,
        app: &mut App,
        request: lume::model::RuleRequest,
    ) {
        let Some(worker) = &self.articles else {
            unavailable(app);
            return;
        };
        let target = request.target.clone();
        match worker.preview(request) {
            Ok(()) => {
                app.rule_preview_started(target);
            }
            Err(error) => app.status = error.to_string(),
        }
    }

    fn receive_feed(&mut self, app: &mut App) -> Result<bool> {
        if self.pending_feed.is_none() {
            self.pending_feed = self.feeds.as_ref().and_then(Worker::poll);
        }
        if !matches!(app.mode, Mode::Normal) || app.selection.is_some() {
            return Ok(false);
        }
        let Some(progress) = self.pending_feed.take() else {
            return Ok(false);
        };
        if let Progress::Updated { completed, total } = progress {
            app.status = format!("Collecting sources · {completed}/{total}");
            return Ok(true);
        }
        app.refreshing = false;
        app.reload()?;
        app.status = match progress {
            Progress::Updated { .. } => "Collecting sources".into(),
            Progress::Finished { feeds, failures } => {
                format!("Collected {feeds} feeds · {failures} failures")
            }
            Progress::Failed(message) => message,
            Progress::Discovered(candidates) => {
                app.discovered(candidates)?;
                "Select a discovered feed to subscribe".into()
            }
        };
        Ok(true)
    }
}

fn unavailable(app: &mut App) {
    app.status = if app.demo {
        "Demo sources are fictional · use lume to build your library".into()
    } else {
        "Network disabled by --offline · cached articles remain available"
            .into()
    };
}
