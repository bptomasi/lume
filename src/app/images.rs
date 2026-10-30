//! Visible-image admission and exact thumbnail keys preserve reader identity.

mod inspector;
mod viewport;
pub use inspector::ImageInspector;

use std::{collections::VecDeque, path::PathBuf};

use anyhow::Result;

use crate::{
    media::{
        ImageDetail, ImageKey, ImageProtocol, ImageSlice, Thumbnail, Worker,
    },
    reader::images::ImageSlot,
};

use super::{Action, App};

/// Current reader slots and a bounded cache of worker-prepared terminal cells.
pub struct ReaderImages {
    /// Non-overlapping source slots in display-row order, for this article.
    pub slots: Vec<ImageSlot>,
    /// Permit loading new images; cached thumbnails remain visible when paused.
    pub enabled: bool,
    worker: Option<Worker>,
    pending: Vec<ImageKey>,
    visible: Vec<ImageKey>,
    deferred: Vec<ImageKey>,
    waiting: bool,
    cache: VecDeque<(ImageKey, Result<Thumbnail, String>)>,
    configuration: Option<(PathBuf, bool)>,
    protocol: ImageProtocol,
    automatic_protocol: ImageProtocol,
    cell_pixels: (u16, u16),
}

impl Default for ReaderImages {
    fn default() -> Self {
        Self {
            slots: Vec::new(),
            enabled: true,
            worker: None,
            pending: Vec::new(),
            visible: Vec::new(),
            deferred: Vec::new(),
            waiting: false,
            cache: VecDeque::new(),
            configuration: None,
            protocol: ImageProtocol::Blocks,
            automatic_protocol: ImageProtocol::Blocks,
            cell_pixels: (10, 20),
        }
    }
}

impl ReaderImages {
    /// Visible references, including captions, in reader display order.
    ///
    /// Reader construction orders non-overlapping slots by their display rows.
    /// Binary bounds keep admission and drawing independent of offscreen images.
    pub fn visible_slots(&self, scroll: usize, height: usize) -> &[ImageSlot] {
        viewport::visible(&self.slots, scroll, height)
    }

    /// Ready cells for this exact URL, viewport and transparency background.
    pub fn thumbnail(&self, key: &ImageKey) -> Option<&Thumbnail> {
        self.cached(key)?.as_ref().ok()
    }

    /// Recoverable loading failure for this exact thumbnail, if observed.
    pub fn error(&self, key: &ImageKey) -> Option<&str> {
        self.cached(key)?.as_ref().err().map(String::as_str)
    }

    /// Explain missing pixels without claiming a stopped worker is loading.
    pub fn placeholder(&self, key: &ImageKey) -> &str {
        if let Some(error) = self.error(key) {
            return error;
        }
        if self.deferred.contains(key) {
            return "Image cache budget reached · scroll to retry · :image";
        }
        if self.worker.is_none() {
            return "Images unavailable · :images retry · :image-open";
        }
        if self.enabled
            || key
                .detail
                .as_ref()
                .is_some_and(|detail| detail.slice.is_none())
        {
            return "Loading image…";
        }
        "Image loading paused"
    }

    fn cached(&self, key: &ImageKey) -> Option<&Result<Thumbnail, String>> {
        self.cache
            .iter()
            .find(|(old, _)| old == key)
            .map(|(_, value)| value)
    }

    fn remember(&mut self, key: ImageKey, result: Result<Thumbnail, String>) {
        self.cache.retain(|(old, _)| *old != key);
        self.cache.push_front((key, result));
        // A viewport can contain all 32 compact formulas, four publisher
        // images and two inspector crops. Avoid evicting visible formula rows.
        self.cache.truncate(40);
        let mut details = 0;
        self.cache.retain(|(key, _)| {
            let inspector = key
                .detail
                .as_ref()
                .is_some_and(|detail| detail.slice.is_none());
            if inspector {
                details += 1;
            }
            !inspector || details <= 2
        });
        let mut bytes = self.retained_bytes();
        while bytes > 32 * 1024 * 1024 {
            let victim = self
                .cache
                .iter()
                .rposition(|(key, _)| !self.visible.contains(key))
                .unwrap_or_else(|| self.cache.len().saturating_sub(1));
            let Some((key, value)) = self.cache.remove(victim) else {
                break;
            };
            bytes = bytes.saturating_sub(entry_bytes(&key, &value));
            if self.visible.contains(&key) && !self.deferred.contains(&key) {
                self.deferred.push(key);
            }
        }
    }

    /// Conservative heap accounting for every prepared image protocol.
    pub fn retained_bytes(&self) -> usize {
        let slots = self.cache.capacity()
            * std::mem::size_of::<(ImageKey, Result<Thumbnail, String>)>();
        slots
            + self
                .cache
                .iter()
                .map(|(key, value)| entry_bytes(key, value))
                .sum::<usize>()
    }

    fn viewport(&mut self, visible: Vec<ImageKey>) -> Vec<ImageKey> {
        if self.visible != visible {
            self.visible = visible;
            self.deferred.clear();
        }
        self.visible
            .iter()
            .filter(|key| {
                self.cached(key).is_none() && !self.deferred.contains(key)
            })
            .cloned()
            .collect()
    }
}

fn entry_bytes(key: &ImageKey, value: &Result<Thumbnail, String>) -> usize {
    key.url.capacity()
        + value.as_ref().map_or_else(String::capacity, |image| {
            image
                .retained_bytes()
                .saturating_sub(std::mem::size_of::<Thumbnail>())
        })
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
        self.images.configuration = Some((directory.clone(), offline));
        self.images.worker = Some(Worker::start(directory, offline)?);
        self.images.pending.clear();
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
        let base = address.and_then(|url| crate::source::feed_url(url).ok());
        resolve_urls(&mut self.images.slots, base.as_ref());
    }

    /// Exact preparation key for an already resolved publisher image slot.
    pub fn image_key(&self, slot: &ImageSlot) -> Option<ImageKey> {
        if slot.url.is_empty()
            || slot.formula.is_some() && !self.uses_sixel_images()
        {
            return None;
        }
        Some(ImageKey {
            url: slot.url.clone(),
            width: self.image_viewport().width.clamp(1, 160) as u16,
            height: slot.rows.len().clamp(1, 48) as u16,
            background: crate::ui::image_background(&self.workspace.theme),
            detail: self.inline_pixel_detail(slot),
        })
    }

    fn inline_pixel_detail(&self, slot: &ImageSlot) -> Option<ImageDetail> {
        if self.images.protocol != ImageProtocol::Sixel {
            return None;
        }
        let viewport = self.image_viewport();
        let bottom = viewport.scroll + viewport.height;
        let eligible = slot.formula.is_some()
            || viewport
                .visible()
                .iter()
                .filter(|image| {
                    image.formula.is_none() && image.rows.start < bottom
                })
                .take(4)
                .any(|image| image.source == slot.source);
        if !eligible {
            return None;
        }
        Some(ImageDetail {
            zoom: 1,
            pan: [0, 0],
            protocol: self.images.protocol,
            cell_pixels: self.images.cell_pixels,
            slice: Some(ImageSlice {
                first_row: 0,
                rows: slot.rows.len().clamp(1, 48) as u16,
            }),
        })
    }

    /// Whether drawing needs a synchronized Sixel frame in the native terminal.
    pub fn uses_sixel_images(&self) -> bool {
        self.images.protocol == ImageProtocol::Sixel
    }

    /// Whether the inspector owns a native pixel surface outside text-cell diffs.
    pub fn uses_native_image_inspector(&self) -> bool {
        matches!(self.mode, super::Mode::Image(_))
            && self.images.protocol != ImageProtocol::Blocks
    }

    /// Retarget source jobs to the visible viewport before admitting more work.
    pub fn poll_images(&mut self) -> bool {
        self.images.waiting = false;
        if self.images.worker.is_none() {
            return false;
        }
        let visible = self.visible_image_keys();
        let wanted = self.images.viewport(visible);
        if let Some(worker) = &mut self.images.worker
            && let Err(error) = worker.retain(&wanted)
        {
            return self.image_worker_failed(error);
        }
        self.images.pending.retain_mut(|pending| {
            let Some(key) = wanted
                .iter()
                .find(|key| *key == pending)
                .or_else(|| wanted.iter().find(|key| key.url == pending.url))
            else {
                return false;
            };
            *pending = key.clone();
            true
        });
        let result = self.images.worker.as_ref().map(Worker::poll);
        let completed = match result {
            Some(Ok(completed)) => completed,
            Some(Err(error)) => return self.image_worker_failed(error),
            None => return false,
        };
        let changed = completed.is_some_and(|completed| {
            self.install_thumbnail(completed, &wanted)
        });
        if self.images.pending.len() < 2 {
            return self.admit_visible_image(&wanted) || changed;
        }
        changed
    }

    fn visible_image_keys(&self) -> Vec<ImageKey> {
        if matches!(self.mode, super::Mode::Image(_)) {
            return self.image_inspector_key().into_iter().collect();
        }
        if self.web_active() {
            if !matches!(self.mode, super::Mode::Web) || self.web.opening {
                return Vec::new();
            }
        } else if self.notebook.is_some()
            || self.navigation_loading
            || crate::ui::workspace_areas(
                self.screen,
                self.focus,
                &self.workspace,
            )
            .reader
            .is_none()
        {
            return Vec::new();
        }
        self.image_viewport()
            .visible()
            .iter()
            .filter(|slot| self.images.enabled || slot.formula.is_some())
            .filter_map(|slot| self.image_key(slot))
            .collect()
    }

    fn image_worker_failed(&mut self, error: anyhow::Error) -> bool {
        self.images.worker = None;
        for key in std::mem::take(&mut self.images.pending) {
            self.images.remember(key, Err(error.to_string()));
        }
        self.status = error.to_string();
        true
    }

    fn install_thumbnail(
        &mut self,
        completed: crate::media::Completion,
        wanted: &[ImageKey],
    ) -> bool {
        self.images
            .pending
            .retain(|key| key.url != completed.key.url);
        if !wanted.contains(&completed.key) {
            return false;
        }
        if let Ok(image) = &completed.result
            && let Some(warning) = &image.cache_warning
        {
            self.status = format!(
                "Image visible · cache unavailable: {}",
                crate::content::label(warning, 200)
            );
        }
        self.images.remember(completed.key, completed.result);
        true
    }

    fn admit_visible_image(&mut self, wanted: &[ImageKey]) -> bool {
        let next = wanted
            .iter()
            .find(|key| {
                self.images.cached(key).is_none()
                    && !self.images.pending.iter().any(|old| old.url == key.url)
            })
            .cloned();
        if let Some(key) = next {
            let generated = self.generated_image(&key);
            if let Some(worker) = &mut self.images.worker {
                match worker.submit(key.clone(), generated) {
                    Ok(true) => self.images.pending.push(key),
                    Ok(false) => self.images.waiting = true,
                    Err(error) => {
                        self.images.remember(key, Err(error.to_string()));
                        return true;
                    }
                }
            }
        }
        false
    }

    fn generated_image(&self, key: &ImageKey) -> Option<std::sync::Arc<[u8]>> {
        if let super::Mode::Image(image) = &self.mode
            && image.url == key.url
            && let Some(formula) = &image.formula
        {
            return Some(formula.svg.clone());
        }
        self.image_viewport()
            .slots
            .iter()
            .find(|slot| slot.url == key.url)?
            .formula
            .as_ref()
            .map(|formula| formula.svg.clone())
    }

    /// Whether image I/O or preparation is admitted in the background.
    pub fn images_pending(&self) -> bool {
        !self.images.pending.is_empty() || self.images.waiting
    }

    pub(super) fn image_command(
        &mut self,
        verb: &str,
        argument: &str,
    ) -> Result<Option<Action>> {
        if verb == "image" {
            self.open_image_inspector()?;
            return Ok(Some(Action::None));
        }
        if verb == "image-protocol" {
            self.set_image_protocol(argument)?;
            return Ok(Some(Action::None));
        }
        if !matches!(verb, "images" | "image-open") {
            return Ok(None);
        }
        if verb == "image-open" {
            return Ok(Some(self.captured_image()?.original_action()));
        }
        match argument {
            "" => {}
            "on" => self.workspace.images.enabled = true,
            "off" => self.workspace.images.enabled = false,
            "retry" => {
                self.images.deferred.clear();
                self.images.cache.retain(|(_, value)| value.is_ok());
                if self.images.worker.is_none()
                    && let Some((directory, offline)) =
                        self.images.configuration.clone()
                {
                    self.enable_images(directory, offline)?;
                }
            }
            _ => anyhow::bail!("Usage: :images [on|off|retry]"),
        }
        if matches!(argument, "on" | "off") {
            self.apply_image_preferences();
            self.persist_preferences()?;
        }
        self.status = format!(
            "Image loading {} · :images on/off · :image-open",
            if self.images.enabled { "on" } else { "paused" }
        );
        Ok(Some(Action::None))
    }
}

pub(super) fn resolve_urls(slots: &mut [ImageSlot], base: Option<&url::Url>) {
    for slot in slots {
        if slot.formula.is_some() {
            continue;
        }
        let url = crate::source::feed_url(&slot.url).ok().or_else(|| {
            let base = base?;
            crate::source::feed_url(base.join(&slot.url).ok()?.as_str()).ok()
        });
        slot.url = url.map_or_else(String::new, |url| url.to_string());
    }
}

#[cfg(test)]
mod tests;
