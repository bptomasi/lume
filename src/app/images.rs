//! Visible-image admission and exact thumbnail keys preserve reader identity.

use std::{collections::VecDeque, path::PathBuf};

use anyhow::{Context, Result, ensure};

use crate::{
    media::{ImageKey, Thumbnail, Worker},
    reader::images::{IMAGE_ROWS, ImageSlot},
};

use super::{Action, App, Focus};

/// Current reader slots and a bounded cache of worker-prepared terminal cells.
pub struct ReaderImages {
    /// Source references and reserved rows of the displayed article only.
    pub slots: Vec<ImageSlot>,
    /// Permit loading new images; cached thumbnails remain visible when paused.
    pub enabled: bool,
    worker: Option<Worker>,
    pending: Option<ImageKey>,
    cache: VecDeque<(ImageKey, Result<Thumbnail, String>)>,
}

impl Default for ReaderImages {
    fn default() -> Self {
        Self {
            slots: Vec::new(),
            enabled: true,
            worker: None,
            pending: None,
            cache: VecDeque::new(),
        }
    }
}

impl ReaderImages {
    /// Ready cells for this exact URL, viewport and transparency background.
    pub fn thumbnail(&self, key: &ImageKey) -> Option<&Thumbnail> {
        self.cached(key)?.as_ref().ok()
    }

    /// Recoverable loading failure for this exact thumbnail, if observed.
    pub fn error(&self, key: &ImageKey) -> Option<&str> {
        self.cached(key)?.as_ref().err().map(String::as_str)
    }

    fn cached(&self, key: &ImageKey) -> Option<&Result<Thumbnail, String>> {
        self.cache
            .iter()
            .find(|(old, _)| old == key)
            .map(|(_, value)| value)
    }
}

impl App {
    /// Start a bounded image worker; offline mode never downloads cache misses.
    ///
    /// # Errors
    /// Returns thread creation failures; text remains usable without images.
    pub fn enable_images(
        &mut self,
        directory: PathBuf,
        offline: bool,
    ) -> Result<()> {
        self.images.worker = Some(Worker::start(directory, offline)?);
        self.images.pending = None;
        Ok(())
    }

    pub(super) fn resolve_image_urls(&mut self) {
        let address = self
            .document
            .as_ref()
            .filter(|_| {
                self.content_source == crate::model::ContentSource::Full
            })
            .map(|document| document.final_url.as_str())
            .filter(|url| !url.is_empty())
            .or_else(|| self.current().map(|article| article.url.as_str()));
        let base =
            address.and_then(|address| crate::source::feed_url(address).ok());
        for slot in &mut self.images.slots {
            let url = crate::source::feed_url(&slot.url).ok().or_else(|| {
                let base = base.as_ref()?;
                crate::source::feed_url(base.join(&slot.url).ok()?.as_str())
                    .ok()
            });
            slot.url = url.map_or_else(String::new, |url| url.to_string());
        }
    }

    /// Exact preparation key for an already resolved publisher image slot.
    pub fn image_key(&self, slot: &ImageSlot) -> Option<ImageKey> {
        if slot.url.is_empty() {
            return None;
        }
        Some(ImageKey {
            url: slot.url.clone(),
            width: self.reader_width.clamp(1, 160) as u16,
            height: IMAGE_ROWS as u16,
            background: crate::ui::image_background(&self.workspace.theme),
        })
    }

    /// Poll prepared cells and submit at most one currently visible image.
    pub fn poll_images(&mut self) -> bool {
        let result = self.images.worker.as_ref().map(Worker::poll);
        let completed = match result {
            Some(Ok(completed)) => completed,
            Some(Err(error)) => {
                self.images.worker = None;
                if let Some(key) = self.images.pending.take() {
                    self.images.cache.push_front((key, Err(error.to_string())));
                    self.images.cache.truncate(24);
                }
                self.status = error.to_string();
                return true;
            }
            None => return false,
        };
        let changed = completed.is_some();
        if let Some(completed) = completed {
            self.install_thumbnail(completed);
        }
        if !self.images.enabled
            || self.images.pending.is_some()
            || self.notebook.is_some()
            || self.navigation_loading
            || !self.workspace.fullscreen && !self.workspace.show_reader
        {
            return changed;
        }
        self.admit_visible_image() || changed
    }

    fn install_thumbnail(&mut self, completed: crate::media::Completion) {
        if let Ok(image) = &completed.result
            && let Some(warning) = &image.cache_warning
        {
            self.status = format!(
                "Image visible · cache unavailable: {}",
                crate::content::label(warning, 200)
            );
        }
        self.images.pending = None;
        self.images
            .cache
            .push_front((completed.key, completed.result));
        self.images.cache.truncate(24);
    }

    fn admit_visible_image(&mut self) -> bool {
        let visible = self.scroll..self.scroll + self.reader_height;
        let next = self
            .images
            .slots
            .iter()
            .filter(|slot| {
                slot.rows.end > visible.start && slot.caption < visible.end
            })
            .filter_map(|slot| self.image_key(slot))
            .find(|key| self.images.cached(key).is_none());
        if let Some(key) = next
            && let Some(worker) = &self.images.worker
        {
            match worker.submit(key.clone()) {
                Ok(()) => self.images.pending = Some(key),
                Err(error) => {
                    self.images.cache.push_front((key, Err(error.to_string())));
                    self.images.cache.truncate(24);
                    return true;
                }
            }
        }
        false
    }

    /// Whether image I/O or preparation is admitted in the background.
    pub fn images_pending(&self) -> bool {
        self.images.pending.is_some()
    }

    pub(super) fn image_command(
        &mut self,
        verb: &str,
        argument: &str,
    ) -> Result<Option<Action>> {
        if !matches!(verb, "images" | "image-open") {
            return Ok(None);
        }
        if verb == "image-open" {
            ensure!(
                self.focus == Focus::Reader && self.notebook.is_none(),
                "Focus Reader before opening an image"
            );
            let slot = self
                .images
                .slots
                .iter()
                .find(|slot| {
                    self.reader_cursor >= slot.caption
                        && self.reader_cursor < slot.rows.end
                })
                .context(
                    "Place the reader cursor on an image or its caption",
                )?;
            let key =
                self.image_key(slot).context("Image URL is not HTTP(S)")?;
            return Ok(Some(Action::Open(key.url)));
        }
        match argument {
            "" => {}
            "on" => self.images.enabled = true,
            "off" => self.images.enabled = false,
            "retry" => self.images.cache.retain(|(_, value)| value.is_ok()),
            _ => anyhow::bail!("Usage: :images [on|off|retry]"),
        }
        self.status = format!(
            "Image loading {} · :images on/off · :image-open",
            if self.images.enabled { "on" } else { "paused" }
        );
        Ok(Some(Action::None))
    }
}
