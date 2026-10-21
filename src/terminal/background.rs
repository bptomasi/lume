//! Independent network lanes and UI-safe delivery of completed background work.

use std::path::PathBuf;

use anyhow::Result;
use lume::{
    app::{App, Mode},
    model::ArticleTarget,
    source::{Progress, Worker, article},
};

pub(super) struct Background {
    configuration: Option<lume::config::Worker>,
    pending_configuration: Option<lume::config::Update>,
    configuration_error: Option<String>,
    feeds: Option<Worker>,
    articles: Option<article::Worker>,
    pending_feed: Option<Progress>,
}

impl Background {
    pub(super) fn start(
        path: PathBuf,
        enabled: bool,
        configuration: Option<lume::config::Worker>,
    ) -> Result<Self> {
        Ok(Self {
            configuration,
            pending_configuration: None,
            configuration_error: None,
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
        dirty |= self.receive_configuration(app);
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

    fn receive_configuration(&mut self, app: &mut App) -> bool {
        let mut dirty = false;
        while let Some(result) = self
            .configuration
            .as_ref()
            .and_then(lume::config::Worker::poll)
        {
            dirty = true;
            match result {
                Ok(update) => {
                    self.pending_configuration = Some(update);
                    self.configuration_error = None;
                }
                Err(error) => {
                    self.configuration_error = Some(format!("{error:#}"))
                }
            }
        }
        if app.configuration_idle()
            && let Some(update) = self.pending_configuration.take()
        {
            dirty = true;
            if let Err(error) = app.configuration_updated(update) {
                self.configuration_error = Some(format!("{error:#}"));
            }
        }
        // An older deferred success must not hide a subsequent save failure.
        if dirty && let Some(error) = &self.configuration_error {
            app.status = error.clone();
        }
        dirty
    }

    pub(super) fn reload_configuration(&self, app: &mut App) {
        let result = self
            .configuration
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Demo configuration is isolated"))
            .and_then(lume::config::Worker::reload);
        app.status = match result {
            Ok(()) => "Configuration reload requested".into(),
            Err(error) => error.to_string(),
        };
    }

    pub(super) fn finish_configuration(&mut self) -> Result<()> {
        if let Some(worker) = self.configuration.take() {
            worker.finish()?;
        }
        Ok(())
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

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use lume::{app::App, config::Session, store::Store};

    use super::Background;

    #[test]
    fn deferred_success_cannot_hide_a_later_configuration_failure() -> Result<()>
    {
        let directory = tempfile::tempdir()?;
        let database = directory.path().join("library.sqlite3");
        let store = Store::open(&database)?;
        let session =
            Session::open(&store, &directory.path().join("config.toml"))?;
        let mut app = App::new(store, false)?;
        app.configure_document(session.initial().clone());
        let mut background = Background::start(database, false, None)?;
        background.pending_configuration = Some(session.initial().clone());
        background.configuration_error = Some("Configuration conflict".into());

        assert!(background.receive_configuration(&mut app));
        assert_eq!(app.status, "Configuration conflict");
        assert!(!background.receive_configuration(&mut app));
        Ok(())
    }
}
