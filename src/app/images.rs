//! Visible-image admission and exact thumbnail keys preserve reader identity.

mod inspector;
pub use inspector::ImageInspector;

use std::{collections::VecDeque, path::PathBuf};

use anyhow::Result;

use crate::{
    media::{
        ImageDetail, ImageKey, ImageProtocol, ImageSlice, Thumbnail, Worker,
    },
    reader::images::{IMAGE_ROWS, ImageSlot},
};

use super::{Action, App};

/// Current reader slots and a bounded cache of worker-prepared terminal cells.
pub struct ReaderImages {
    /// Non-overlapping source slots in display-row order, for this article.
    pub slots: Vec<ImageSlot>,
    /// Permit loading new images; cached thumbnails remain visible when paused.
    pub enabled: bool,
    worker: Option<Worker>,
    pending: Option<ImageKey>,
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
            pending: None,
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
        if height == 0 {
            return &[];
        }
        let first = self.slots.partition_point(|slot| slot.rows.end <= scroll);
        let remaining = &self.slots[first..];
        let bottom = scroll.saturating_add(height);
        let count = remaining.partition_point(|slot| slot.caption < bottom);
        &remaining[..count]
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
        if self.worker.is_none() {
            return "Images unavailable · :images retry · :image-open";
        }
        if self.enabled || key.detail.is_some() {
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
        self.cache.truncate(24);
        let mut details = 0;
        let mut bytes = 0;
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
        self.cache.retain(|(_, value)| {
            let size = value.as_ref().ok().map_or(0, |image| {
                image.rows.as_ref().map_or(0, |rows| rows.bytes())
            });
            if bytes + size > 32 * 1024 * 1024 {
                return false;
            }
            bytes += size;
            true
        });
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
        self.images.configuration = Some((directory.clone(), offline));
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
            detail: self.inline_pixel_detail(slot),
        })
    }

    fn inline_pixel_detail(&self, slot: &ImageSlot) -> Option<ImageDetail> {
        if self.images.protocol != ImageProtocol::Sixel {
            return None;
        }
        let bottom = self.scroll + self.reader_height;
        let eligible = self
            .images
            .visible_slots(self.scroll, self.reader_height)
            .iter()
            .filter(|image| image.rows.start < bottom)
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
                rows: IMAGE_ROWS as u16,
            }),
        })
    }

    /// Whether drawing needs a synchronized Sixel frame in the native terminal.
    pub fn uses_sixel_images(&self) -> bool {
        self.images.protocol == ImageProtocol::Sixel
    }

    /// Poll prepared cells and submit at most one currently visible image.
    pub fn poll_images(&mut self) -> bool {
        let result = self.images.worker.as_ref().map(Worker::poll);
        let completed = match result {
            Some(Ok(completed)) => completed,
            Some(Err(error)) => {
                self.images.worker = None;
                if let Some(key) = self.images.pending.take() {
                    self.images.remember(key, Err(error.to_string()));
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
        if !self.can_request_images() {
            return changed;
        }
        self.admit_visible_image() || changed
    }

    fn can_request_images(&self) -> bool {
        if self.images.pending.is_some()
            || self.notebook.is_some()
            || self.navigation_loading
        {
            return false;
        }
        if matches!(self.mode, super::Mode::Image(_)) {
            return true;
        }
        self.images.enabled
            && crate::ui::workspace_areas(
                self.screen,
                self.focus,
                &self.workspace,
            )
            .reader
            .is_some()
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
        self.images.remember(completed.key, completed.result);
    }

    fn admit_visible_image(&mut self) -> bool {
        let next = self.image_inspector_key().or_else(|| {
            self.images
                .visible_slots(self.scroll, self.reader_height)
                .iter()
                .filter_map(|slot| self.image_key(slot))
                .find(|key| self.images.cached(key).is_none())
        });
        let next = next.filter(|key| self.images.cached(key).is_none());
        if let Some(key) = next
            && let Some(worker) = &self.images.worker
        {
            match worker.submit(key.clone()) {
                Ok(()) => self.images.pending = Some(key),
                Err(error) => {
                    self.images.remember(key, Err(error.to_string()));
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
            return Ok(Some(Action::Open(self.captured_image()?.url)));
        }
        match argument {
            "" => {}
            "on" => self.images.enabled = true,
            "off" => self.images.enabled = false,
            "retry" => {
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
        self.status = format!(
            "Image loading {} · :images on/off · :image-open",
            if self.images.enabled { "on" } else { "paused" }
        );
        Ok(Some(Action::None))
    }
}
