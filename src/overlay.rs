//! Optional window-system projection uses only Lume's prepared local PNG files.
//!
//! Desired frames replace stale frames. A worker owns persistent X11 surfaces or
//! the optional helper and timeout-bounded writes; drawing performs no display I/O.
//! Removing or replacing a projection releases its file only after publication.

mod launch;
mod process;
#[cfg(unix)]
mod process_control;
mod projection;
#[cfg(unix)]
mod x11;

use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use anyhow::Result;
use ratatui::layout::Rect;

use crate::{latest::Latest, media::PreparedPng};

/// Display backend supported by the separately installed Überzug++ helper.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Output {
    /// An X11 or XWayland terminal window.
    X11,
    /// A compositor explicitly supported by the helper.
    Wayland,
}

/// Position of the terminal grid inside its X11 client window, in pixels.
#[derive(Clone, Copy)]
pub enum GridOrigin {
    /// Explicit left/top padding; Alacritty defaults to zero padding.
    Fixed([u16; 2]),
    /// Symmetric dynamic padding computed from the remaining window pixels.
    Centered,
}

/// Captured helper identity and display backend; no command is shell evaluated.
#[derive(Clone)]
pub struct Configuration {
    /// Absolute path to the user's installed executable.
    pub executable: PathBuf,
    /// Qualified window-system backend.
    pub output: Output,
    /// Optional installed process isolator for a Linux X11 terminal.
    pub isolation: Option<PathBuf>,
}

/// One clipped, prepared image and its terminal-cell placement.
#[derive(Clone)]
pub struct Placement {
    /// Instance-local identity used for explicit removal.
    pub id: String,
    /// Visible terminal cells, already clipped at the owning pane.
    pub area: Rect,
    /// Owned PNG; publisher bytes and URLs never reach the helper.
    pub image: Arc<PreparedPng>,
    /// Optional vertical source-pixel range, cropped on the presentation worker.
    pub crop: Option<std::ops::Range<u32>>,
}

impl PartialEq for Placement {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.area == other.area
            && self.image.path() == other.image.path()
            && self.crop == other.crop
    }
}

/// Latest desired overlay frame, independent of reader/annotation persistence.
pub struct Runtime {
    worker: Latest<Request, ()>,
    staged: Vec<Placement>,
    submitted: Vec<Placement>,
    ready_at: Option<Instant>,
    health_at: Option<Instant>,
    validity: Option<Arc<AtomicBool>>,
    geometry: Option<Geometry>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Geometry {
    cells: (u16, u16),
    pixels: (u16, u16),
}

struct Request {
    desired: Option<Vec<Placement>>,
    valid: Arc<AtomicBool>,
    geometry: Option<Geometry>,
}

impl Runtime {
    /// Start an idle presentation lane; the helper starts only for actual pixels.
    ///
    /// # Errors
    /// Returns thread creation failure; reading does not depend on this lane.
    pub fn start(configuration: Result<Configuration, String>) -> Result<Self> {
        let mut layer = process::Layer::new(configuration);
        Self::present(move |desired| layer.apply(desired))
    }

    /// Start persistent child windows in the captured X11 terminal.
    ///
    /// # Errors
    /// Returns thread creation failure. Display failures arrive through `poll`.
    #[cfg(unix)]
    pub fn start_x11(
        display: String,
        window: u32,
        origin: GridOrigin,
    ) -> Result<Self> {
        let mut layer = x11::Layer::new(display, window, origin);
        Self::present(move |desired| layer.apply(desired))
    }

    fn present(
        operation: impl FnMut(Request) -> Result<()> + Send + 'static,
    ) -> Result<Self> {
        Ok(Self {
            worker: Latest::start("lume-image-overlay", operation)?,
            staged: Vec::new(),
            submitted: Vec::new(),
            ready_at: None,
            health_at: None,
            validity: None,
            geometry: None,
        })
    }

    /// Replace captured helper geometry when terminal cells or font size change.
    pub fn resize(&mut self, cells: (u16, u16), pixels: (u16, u16)) {
        let geometry = Some(Geometry { cells, pixels });
        if self.geometry == geometry {
            return;
        }
        self.geometry = geometry;
        self.staged.clear();
        self.submitted.clear();
        self.ready_at = None;
        self.health_at = None;
        self.submit(None);
    }

    /// Replace the desired frame without postponing an existing admission time.
    pub fn stage(&mut self, desired: Vec<Placement>, now: Instant) {
        if self.staged == desired {
            return;
        }
        self.staged = desired;
        self.health_at = None;
        if self.staged.is_empty() {
            self.ready_at = None;
            self.submit(None);
            self.submitted.clear();
        } else {
            // Throttle rather than debounce: continued scrolling must not
            // starve publication or repeatedly restart an active helper.
            self.ready_at.get_or_insert(now + Duration::from_millis(16));
        }
    }

    /// Publish the latest frame at bounded frequency without blocking input.
    ///
    /// # Errors
    /// Returns helper startup, transport, exit or file-publication errors.
    pub fn poll(&mut self, now: Instant) -> Result<()> {
        self.worker.poll().transpose()?;
        if self.ready_at.is_some_and(|deadline| now >= deadline)
            && !self.worker.pending()
        {
            self.ready_at = None;
            self.submitted = self.staged.clone();
            self.submit(Some(self.submitted.clone()));
            self.health_at = Some(now + Duration::from_secs(1));
        }
        if self.health_at.is_some_and(|deadline| now >= deadline)
            && !self.worker.pending()
        {
            self.health_at = Some(now + Duration::from_secs(1));
            self.submit(Some(self.submitted.clone()));
        }
        Ok(())
    }

    /// Whether presentation work or the short admission delay remains pending.
    pub fn pending(&self) -> bool {
        self.ready_at.is_some() || self.worker.pending()
    }

    fn submit(&mut self, desired: Option<Vec<Placement>>) {
        self.invalidate();
        let valid = Arc::new(AtomicBool::new(true));
        self.worker.submit(Request {
            desired,
            valid: valid.clone(),
            geometry: self.geometry,
        });
        self.validity = Some(valid);
    }

    fn invalidate(&mut self) {
        if let Some(validity) = self.validity.take() {
            validity.store(false, Ordering::Release);
        }
    }

    /// Hide and stop the helper at an explicit suspension/shutdown boundary.
    ///
    /// # Errors
    /// Returns worker errors or a bounded cleanup timeout.
    pub fn suspend(&mut self) -> Result<()> {
        self.staged.clear();
        self.submitted.clear();
        self.ready_at = None;
        self.health_at = None;
        self.submit(None);
        let deadline = Instant::now() + Duration::from_millis(500);
        while self.worker.pending() {
            self.worker.poll().transpose()?;
            anyhow::ensure!(
                Instant::now() < deadline,
                "Image helper cleanup timed out"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        Ok(())
    }
}

fn validate(desired: &[Placement]) -> Result<()> {
    anyhow::ensure!(desired.len() <= 4, "Overlay frame exceeds four images");
    anyhow::ensure!(
        desired.iter().all(|image| image.id.len() <= 128
            && !image.area.is_empty()
            && image.area.width <= 160
            && image.area.height <= 48),
        "Invalid overlay placement"
    );
    Ok(())
}

impl Drop for Runtime {
    fn drop(&mut self) {
        self.invalidate();
    }
}
