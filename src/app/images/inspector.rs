//! A captured image owns its viewport; closing restores untouched reader state.

use anyhow::{Context, Result, bail, ensure};
use crossterm::event::{KeyCode, KeyEvent};

use crate::{
    app::{Action, App, Focus, Mode},
    media::{ImageDetail, ImageKey, ImageProtocol},
};

/// Captured publisher image, independent of later selection or feed refresh.
#[derive(Clone, Debug)]
pub struct ImageInspector {
    /// Validated absolute publisher URL.
    pub url: String,
    /// Plain alternative description used as the inspector title.
    pub title: String,
    /// Source-based magnification, from one through eight.
    pub zoom: u8,
    /// Overflow position in horizontal/vertical permille, from zero to 1000.
    pub pan: [u16; 2],
    /// Captured local outlines survive navigation and never become an HTTP URL.
    pub formula: Option<crate::reader::FormulaImage>,
}

impl ImageInspector {
    pub(in crate::app) fn original_action(&self) -> Action {
        self.formula.as_ref().map_or_else(
            || Action::Open(self.url.clone()),
            |formula| Action::Copy(formula.latex.to_string()),
        )
    }
}

impl App {
    /// Configure detected native rendering without reading terminal input.
    pub fn configure_image_terminal(
        &mut self,
        protocol: ImageProtocol,
        cell_pixels: (u16, u16),
    ) {
        self.images.automatic_protocol = protocol;
        self.apply_image_preferences();
        self.update_image_cell_pixels(cell_pixels);
    }

    pub(in crate::app) fn apply_image_preferences(&mut self) {
        use crate::model::ImagePreference;
        self.images.enabled = self.workspace.images.enabled;
        self.images.protocol = match self.workspace.images.protocol {
            ImagePreference::Auto => self.images.automatic_protocol,
            ImagePreference::Blocks => ImageProtocol::Blocks,
            ImagePreference::Sixel => ImageProtocol::Sixel,
            ImagePreference::Iterm2 => ImageProtocol::Iterm2,
        };
        self.apply_formula_preferences();
    }

    /// Update OS-reported cell geometry; unusable reports fall back to 10×20.
    pub fn update_image_cell_pixels(&mut self, pixels: (u16, u16)) {
        self.images.cell_pixels =
            if (1..=64).contains(&pixels.0) && (1..=128).contains(&pixels.1) {
                pixels
            } else {
                (10, 20)
            };
    }

    pub(in crate::app) fn captured_image(&self) -> Result<ImageInspector> {
        self.guard_stale_command("image")?;
        ensure!(
            self.focus == Focus::Reader && self.notebook.is_none(),
            "Focus Reader before opening an image"
        );
        let slot = self
            .images
            .visible_slots(self.reader_cursor, 1)
            .iter()
            .find(|slot| {
                self.reader_cursor >= slot.caption
                    && self.reader_cursor < slot.rows.end
            })
            .context("Place the reader cursor on an image or its caption")?;
        let key = self.image_key(slot).context("Image URL is not HTTP(S)")?;
        Ok(ImageInspector {
            url: key.url,
            title: slot.alt.clone(),
            formula: slot.formula.clone(),
            zoom: 1,
            pan: [0, 0],
        })
    }

    pub(crate) fn open_image_inspector(&mut self) -> Result<()> {
        let image = self.captured_image()?;
        self.vim.clear_input();
        self.mode = Mode::Image(Box::new(image));
        Ok(())
    }

    /// Exact inspector request, including source crop and terminal cell size.
    pub fn image_inspector_key(&self) -> Option<ImageKey> {
        let Mode::Image(image) = &self.mode else {
            return None;
        };
        let area = crate::ui::image_inspector_area(self.screen);
        let mut width = area.width.max(1);
        let height = area.height.max(1);
        let cell = self.images.cell_pixels;
        if self.images.protocol != ImageProtocol::Blocks {
            let column_pixels = u64::from(height)
                * u64::from(cell.0.max(1))
                * u64::from(cell.1.max(1));
            width =
                (4_000_000 / column_pixels).clamp(1, u64::from(width)) as u16;
        }
        Some(ImageKey {
            url: image.url.clone(),
            width,
            height,
            background: crate::ui::image_background(&self.workspace.theme),
            detail: Some(ImageDetail {
                zoom: image.zoom,
                pan: image.pan,
                protocol: self.images.protocol,
                cell_pixels: cell,
                slice: None,
            }),
        })
    }

    pub(in crate::app) fn set_image_protocol(
        &mut self,
        argument: &str,
    ) -> Result<()> {
        use crate::model::ImagePreference;
        let preference = match argument {
            "" => self.workspace.images.protocol,
            "auto" => ImagePreference::Auto,
            "blocks" => ImagePreference::Blocks,
            "sixel" => ImagePreference::Sixel,
            "iterm2" => ImagePreference::Iterm2,
            _ => bail!("Usage: :image-protocol [auto|blocks|sixel|iterm2]"),
        };
        if self.workspace.images.protocol != preference {
            self.workspace.images.protocol = preference;
            self.persist_preferences()?;
        }
        self.apply_image_preferences();
        self.status = format!(
            "Images: {} · :image opens · :image-open shows original",
            self.images.protocol.label(),
        );
        Ok(())
    }

    pub(in crate::app) fn image_inspector_keypress(
        &mut self,
        key: KeyEvent,
    ) -> Result<Action> {
        let Mode::Image(image) = &mut self.mode else {
            return Ok(Action::None);
        };
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.mode = Mode::Normal,
            KeyCode::Char('y') if image.formula.is_some() => {
                return Ok(image.original_action());
            }
            KeyCode::Char('o') => return Ok(image.original_action()),
            KeyCode::Char('+') | KeyCode::Char('=') => {
                image.zoom = image.zoom.saturating_add(1).min(8);
            }
            KeyCode::Char('-') => {
                image.zoom = image.zoom.saturating_sub(1).max(1);
                if image.zoom == 1 {
                    image.pan = [0, 0];
                }
            }
            KeyCode::Char('0') | KeyCode::Home => {
                image.zoom = 1;
                image.pan = [0, 0];
            }
            KeyCode::Left | KeyCode::Char('h') => pan(image, 0, false, 1),
            KeyCode::Right | KeyCode::Char('l') => pan(image, 0, true, 1),
            KeyCode::Up | KeyCode::Char('k') => pan(image, 1, false, 1),
            KeyCode::Down | KeyCode::Char('j') => pan(image, 1, true, 1),
            KeyCode::Char('b') => {
                self.images.protocol = ImageProtocol::Blocks;
                self.apply_formula_preferences();
            }
            _ => {}
        }
        Ok(Action::None)
    }

    pub(in crate::app) fn pan_image(&mut self, forward: bool, amount: usize) {
        if let Mode::Image(image) = &mut self.mode {
            pan(image, 1, forward, amount);
        }
    }
}

fn pan(image: &mut ImageInspector, axis: usize, forward: bool, amount: usize) {
    if image.zoom == 1 {
        return;
    }
    let distance = amount.saturating_mul(50).min(1000) as u16;
    image.pan[axis] = if forward {
        image.pan[axis].saturating_add(distance).min(1000)
    } else {
        image.pan[axis].saturating_sub(distance)
    };
}
