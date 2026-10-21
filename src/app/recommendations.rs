//! Local recommendations run outside input, with ordered bounded preference writes.
//!
//! The adapter admits edits without replacement; refresh requests are coalesced.
//! Shutdown drains admitted edits but need not finish disposable classification.
//! SQLite's durable dirty queue resumes unfinished work in the next session.

mod commands;
mod worker;

use super::{App, Mode, View};
use crate::model::{
    ArticlePage, ArticleQuery, Interest, RecommendationFeedback,
    RecommendationProfile,
};
use anyhow::{Result, ensure};
use std::time::{Duration, Instant};

#[derive(Clone)]
pub(super) enum Edit {
    Interest(Interest),
    Remove(String),
    Exclude(String),
    Minutes(Option<u16>),
    IncludeRead(bool),
    Source(i64, i8),
    Reset,
    Feedback(i64, Option<RecommendationFeedback>),
}

impl Edit {
    fn validate(&self) -> Result<()> {
        let mut profile = RecommendationProfile::default();
        match self {
            Self::Interest(interest) => {
                profile.interests.push(interest.clone())
            }
            Self::Exclude(query) => profile.exclude = query.clone(),
            Self::Minutes(minutes) => profile.minutes = *minutes,
            Self::Source(id, weight) => {
                profile.sources.insert(*id, *weight);
            }
            _ => return Ok(()),
        }
        crate::recommendation::validate_profile(&profile)
    }
}

pub(super) struct Recommendations {
    worker: worker::Worker,
    next_check: Instant,
    refresh_needed: bool,
    ready_page: Option<(ArticleQuery, ArticlePage)>,
}

impl Recommendations {
    pub(super) fn start(path: &std::path::Path) -> Result<Self> {
        Ok(Self {
            worker: worker::Worker::start(path)?,
            next_check: Instant::now(),
            refresh_needed: true,
            ready_page: None,
        })
    }
}

impl App {
    /// Apply completed local ranking; never replace an active reader or draft.
    ///
    /// # Errors
    /// Reports rejected edits or background SQLite failures, retaining cached data.
    pub fn poll_recommendations(&mut self) -> Result<bool> {
        let query = self.recommendation_query();
        let Some(state) = &mut self.recommendations else {
            return Ok(false);
        };
        let mut changed = false;
        while let Some(result) = state.worker.poll() {
            let result = result?;
            let update = result.update;
            if result.page.is_some() {
                state.ready_page = result.page;
            }
            state.refresh_needed = update.remaining;
            if let Some(row) = self
                .navigation
                .iter_mut()
                .find(|row| row.view == View::Recommended)
            {
                changed |= row.count != update.total;
                row.count = update.total;
            }
        }
        if !state.worker.pending() && Instant::now() >= state.next_check {
            state.worker.submit(None, query.clone())?;
            state.next_check = Instant::now() + Duration::from_secs(2);
        } else if !state.worker.pending() && state.refresh_needed {
            state.worker.submit(None, query.clone())?;
        }
        if self.install_ready_recommendations()? {
            changed = true;
        }
        Ok(changed)
    }

    fn recommendation_query(&self) -> Option<ArticleQuery> {
        (self.view == View::Recommended && self.notebook.is_none()).then(|| {
            ArticleQuery {
                view: View::Recommended,
                text: self.search.clone(),
                offset: self.page_offset,
                limit: self.page_size.clamp(1, 500),
                retained: self.current().map(|article| article.id),
            }
        })
    }

    fn install_ready_recommendations(&mut self) -> Result<bool> {
        if self.notebook.is_some()
            || self.focus == super::Focus::Reader
            || !matches!(self.mode, Mode::Normal)
            || self.selection.is_some()
            || self.navigation_loading
        {
            return Ok(false);
        }
        let page = self
            .recommendations
            .as_mut()
            .and_then(|state| state.ready_page.take());
        if let Some((query, page)) = page {
            if page.total == self.page_total
                && page.articles == self.library.articles
            {
                return Ok(false);
            }
            self.install_metadata_page(query, page)?;
            return Ok(true);
        }
        Ok(false)
    }

    /// Whether admitted recommendation work is still active.
    pub fn recommendations_pending(&self) -> bool {
        self.recommendations
            .as_ref()
            .is_some_and(|state| state.worker.pending())
    }

    pub(super) fn flush_recommendations(&mut self) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(5);
        let Some(state) = &mut self.recommendations else {
            return Ok(());
        };
        let mut failure = None;
        loop {
            while let Some(result) = state.worker.poll() {
                if let Err(error) = result {
                    failure = Some(error);
                }
            }
            if !state.worker.pending() {
                break;
            }
            ensure!(
                Instant::now() < deadline,
                "Recommendation writes did not finish"
            );
            std::thread::sleep(Duration::from_millis(2));
        }
        if let Some(error) = failure {
            return Err(error);
        }
        Ok(())
    }

    fn edit_recommendations(&mut self, edit: Edit) -> Result<()> {
        edit.validate()?;
        let query = self.recommendation_query();
        if let Some(state) = &mut self.recommendations {
            state.worker.submit(Some(edit), query)?;
            self.status =
                "Recommendation update queued · navigation remains available"
                    .into();
        } else {
            apply_edit(&self.store, edit)?;
            self.store.refresh_recommendations()?;
            self.rebuild_navigation()?;
            self.status = "Recommendation preferences saved".into();
        }
        Ok(())
    }
}

fn apply_edit(store: &crate::store::Store, edit: Edit) -> Result<()> {
    if let Edit::Feedback(article, feedback) = edit {
        return store.set_recommendation_feedback(article, feedback);
    }
    let mut profile = store.recommendation_profile()?;
    match edit {
        Edit::Interest(interest) => {
            profile
                .interests
                .retain(|item| !item.name.eq_ignore_ascii_case(&interest.name));
            profile.interests.push(interest);
        }
        Edit::Remove(name) => {
            ensure!(
                profile
                    .interests
                    .iter()
                    .any(|item| item.name.eq_ignore_ascii_case(&name)),
                "Unknown interest: {name}"
            );
            profile
                .interests
                .retain(|item| !item.name.eq_ignore_ascii_case(&name));
        }
        Edit::Exclude(query) => profile.exclude = query,
        Edit::Minutes(minutes) => profile.minutes = minutes,
        Edit::IncludeRead(value) => profile.include_read = value,
        Edit::Source(id, weight) => {
            if weight == 0 {
                profile.sources.remove(&id);
            } else {
                profile.sources.insert(id, weight);
            }
        }
        Edit::Reset => profile = RecommendationProfile::default(),
        Edit::Feedback(_, _) => return Ok(()),
    }
    store.save_recommendation_profile(&profile)
}
