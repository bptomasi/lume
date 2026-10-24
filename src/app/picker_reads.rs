//! Independent latest-query and latest-preview lanes keep selectors responsive.
//!
//! Connections open lazily on their owning threads. Opening a selector cancels
//! the previous generation; results never publish into a later selector.

use std::{collections::VecDeque, path::PathBuf};

use anyhow::{Context, Result};

use crate::{latest::Latest, store::Store};

use super::{App, Choice, ChoiceAction, Mode, PickerKind};

pub(super) struct PickerReads {
    list: Latest<String, Vec<Choice>>,
    preview: Latest<i64, String>,
    query: Option<String>,
    article: Option<i64>,
    cache: VecDeque<(i64, String)>,
}

impl PickerReads {
    fn start(path: PathBuf) -> Result<Self> {
        let preview_path = path.clone();
        let mut list_store = None;
        let mut preview_store = None;
        Ok(Self {
            list: Latest::start_cancellable(
                "lume-picker-search",
                move |query: String, cancel| {
                    connection(&mut list_store, &path)?
                        .read_cancellable(cancel, |store| {
                            super::picker::article_choices(store, &query)
                        })
                },
            )?,
            preview: Latest::start_cancellable(
                "lume-picker-preview",
                move |article, cancel| {
                    connection(&mut preview_store, &preview_path)?
                        .read_cancellable(cancel, |store| {
                            let body = store.body(article)?;
                            Ok(body.chars().take(6000).collect())
                        })
                },
            )?,
            query: None,
            article: None,
            cache: VecDeque::new(),
        })
    }

    fn reset(&mut self) {
        self.list.cancel();
        self.preview.cancel();
        self.query = None;
        self.article = None;
        self.cache.clear();
    }
}

fn connection<'a>(
    store: &'a mut Option<Store>,
    path: &std::path::Path,
) -> Result<&'a Store> {
    if store.is_none() {
        *store = Some(Store::open(path)?);
    }
    store.as_ref().context("Selector connection unavailable")
}

impl App {
    pub(super) fn reset_picker_reads(&mut self) -> Result<()> {
        if self.navigation_worker.is_none() {
            return Ok(());
        }
        if self.picker_reads.is_none() {
            self.picker_reads =
                Some(PickerReads::start(self.store.database_path()?)?);
        }
        if let Some(reads) = &mut self.picker_reads {
            reads.reset();
        }
        Ok(())
    }

    pub(super) fn update_article_picker(&mut self) -> bool {
        let (Mode::Picker(picker), Some(reads)) =
            (&mut self.mode, &mut self.picker_reads)
        else {
            return false;
        };
        if !matches!(picker.kind, PickerKind::Articles) {
            return false;
        }
        if reads.query.as_deref() != Some(&picker.query) {
            self.performance.begin(super::Operation::Search);
            reads.query = Some(picker.query.clone());
            reads.list.submit(picker.query.clone());
            reads.preview.cancel();
            reads.article = None;
            picker.choices.clear();
            picker.selected = 0;
            picker.preview = "Searching…".into();
        } else {
            request_preview(picker, reads, &mut self.performance);
        }
        true
    }

    /// Deliver current selector results without querying SQLite on input.
    ///
    /// # Errors
    /// Returns the current query/preview failure; stale results are discarded.
    pub fn poll_picker(&mut self) -> Result<bool> {
        let Some(reads) = &mut self.picker_reads else {
            return Ok(false);
        };
        let Mode::Picker(picker) = &mut self.mode else {
            reads.reset();
            return Ok(false);
        };
        if !matches!(picker.kind, PickerKind::Articles) {
            reads.reset();
            return Ok(false);
        }
        let mut changed = false;
        if let Some(result) = reads.list.poll() {
            self.performance.finish(super::Operation::Search);
            picker.choices = result.map_err(|error| {
                picker.preview = format!("Search failed: {error:#}");
                error
            })?;
            picker.selected =
                picker.selected.min(picker.choices.len().saturating_sub(1));
            request_preview(picker, reads, &mut self.performance);
            changed = true;
        }
        if let Some(result) = reads.preview.poll() {
            self.performance.finish(super::Operation::Preview);
            let body = result.map_err(|error| {
                picker.preview = format!("Preview failed: {error:#}");
                error
            })?;
            if let Some(article) = reads.article {
                reads.cache.push_back((article, body.clone()));
                if reads.cache.len() > 16 {
                    reads.cache.pop_front();
                }
            }
            picker.preview = body;
            changed = true;
        }
        Ok(changed)
    }

    /// Whether selector queries or previews await delivery.
    pub fn picker_pending(&self) -> bool {
        self.picker_reads.as_ref().is_some_and(|reads| {
            reads.list.pending() || reads.preview.pending()
        })
    }
}

fn request_preview(
    picker: &mut super::Picker,
    reads: &mut PickerReads,
    performance: &mut super::performance::Performance,
) {
    let target = picker.choices.get(picker.selected).and_then(|choice| {
        if let ChoiceAction::JumpArticle(article) = choice.action {
            Some(article)
        } else {
            None
        }
    });
    let Some(article) = target else {
        reads.preview.cancel();
        reads.article = None;
        picker.preview = "No matches · refine your search".into();
        return;
    };
    if target == reads.article {
        return;
    }
    reads.article = target;
    reads.preview.cancel();
    if let Some((_, body)) = reads.cache.iter().find(|(id, _)| *id == article) {
        picker.preview.clone_from(body);
    } else {
        picker.preview = "Loading preview…".into();
        performance.begin(super::Operation::Preview);
        reads.preview.submit(article);
    }
}
