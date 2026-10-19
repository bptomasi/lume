//! Foreground child ownership and bounded presentation transport.

use anyhow::{Context, Result};

use super::{Configuration, Geometry, Placement, Request};
use std::sync::atomic::{AtomicBool, Ordering};

pub(super) struct Layer {
    configuration: Result<Configuration, String>,
    session: Option<Session>,
    geometry: Option<Geometry>,
    projection: super::projection::Projection,
}

impl Layer {
    pub(super) fn new(configuration: Result<Configuration, String>) -> Self {
        Self {
            configuration,
            session: None,
            geometry: None,
            projection: super::projection::Projection::default(),
        }
    }

    pub(super) fn apply(&mut self, request: Request) -> Result<()> {
        if !request.valid.load(Ordering::Acquire) {
            return Ok(());
        }
        let Some(desired) = request.desired else {
            self.session = None;
            self.projection = Default::default();
            return Ok(());
        };
        super::validate(&desired)?;
        let desired = desired
            .into_iter()
            .map(|placement| self.projection.prepare(placement))
            .collect::<Result<Vec<_>>>()?;
        if self.geometry != request.geometry {
            self.session = None;
            self.geometry = request.geometry;
        }
        if desired.is_empty() && self.session.is_none() {
            return Ok(());
        }
        if self.session.is_none() {
            let configuration = self
                .configuration
                .as_ref()
                .map_err(|error| anyhow::anyhow!("{error}"))?;
            self.session = Some(Session::start(configuration)?);
        }
        let result = self
            .session
            .as_mut()
            .context("Image helper did not start")?
            .sync(desired, &request.valid);
        // Cancellation may interrupt a multi-image publication after one add.
        // Stop that session so unacknowledged partial frames cannot survive.
        if result.is_err() || !request.valid.load(Ordering::Acquire) {
            self.session = None;
        }
        result
    }
}

#[cfg(unix)]
struct Session {
    child: super::process_control::Process,
    input: std::process::ChildStdin,
    current: Vec<Placement>,
    previous: Vec<Placement>,
    _directory: tempfile::TempDir,
}

#[cfg(unix)]
impl Session {
    fn start(configuration: &Configuration) -> Result<Self> {
        use std::time::Duration;
        let directory =
            tempfile::Builder::new().prefix("lume-overlay-").tempdir()?;
        let child = super::launch::command(configuration, directory.path())
            .spawn()
            .context("Install ueberzugpp 2.9.10 to enable overlay pixels")?;
        let child = super::process_control::Process::start(child)?;
        let input = child.take_stdin()?;
        let session = Self {
            child,
            input,
            current: Vec::new(),
            previous: Vec::new(),
            _directory: directory,
        };
        // Observe immediate loader/display failures before acknowledging startup.
        std::thread::sleep(Duration::from_millis(40));
        Ok(session)
    }

    fn sync(
        &mut self,
        desired: Vec<Placement>,
        valid: &AtomicBool,
    ) -> Result<()> {
        if !valid.load(Ordering::Acquire) {
            return Ok(());
        }
        self.child.running()?;
        let mut commands = Vec::new();
        for old in &self.current {
            if !desired.iter().any(|image| image.id == old.id) {
                commands.push(serde_json::json!({
                    "action": "remove",
                    "identifier": old.id,
                }));
            }
        }
        for image in &desired {
            if !self.current.contains(image) {
                commands.push(serde_json::json!({
                    "action": "add",
                    "identifier": image.id,
                    "x": image.area.x,
                    "y": image.area.y,
                    "max_width": image.area.width,
                    "max_height": image.area.height,
                    "path": image.image.path().to_str().context("Image path is not UTF-8")?,
                    // PNG pixels already fit; never enlarge rounded cell bounds.
                    "scaler": "contain",
                }));
            }
        }
        for command in commands {
            if !valid.load(Ordering::Acquire) {
                return Ok(());
            }
            self.send(&command)?;
        }
        // Preserve one previous projection while the helper consumes its input.
        self.previous = std::mem::replace(&mut self.current, desired);
        Ok(())
    }

    fn send(&mut self, command: &serde_json::Value) -> Result<()> {
        use std::io::Write;
        let mut bytes = serde_json::to_vec(command)?;
        bytes.push(b'\n');
        self.child.begin_write()?;
        let result = self.input.write_all(&bytes);
        let finished = self.child.end_write();
        result.context("Image helper stopped accepting frames")?;
        finished
    }
}

#[cfg(not(unix))]
struct Session;

#[cfg(not(unix))]
impl Session {
    fn start(_: &Configuration) -> Result<Self> {
        anyhow::bail!("Window-system overlays require a Unix platform")
    }

    fn sync(&mut self, _: Vec<Placement>, _: &AtomicBool) -> Result<()> {
        anyhow::bail!("Window-system overlays require a Unix platform")
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::{
        fs,
        os::unix::fs::PermissionsExt,
        sync::Arc,
        time::{Duration, Instant},
    };

    #[test]
    fn a_blocked_helper_pipe_times_out_and_the_process_is_reaped() -> Result<()>
    {
        let directory = tempfile::tempdir()?;
        let executable = directory.path().join("ueberzugpp");
        fs::write(
            &executable,
            include_str!("../../tests/fixtures/images/overlay_helper.py"),
        )?;
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o755))?;
        fs::write(directory.path().join("mode"), "stall")?;
        let mut session = Session::start(&Configuration {
            executable,
            output: super::super::Output::X11,
            isolation: None,
        })?;
        // A large valid JSON value fills ordinary pipe capacities. This probes
        // transport failure, independently of the small production frame size.
        let command =
            serde_json::json!({ "transport_fixture": "x".repeat(1_000_000) });
        let started = Instant::now();
        let result = session.send(&command);
        assert!(result.is_err());
        assert!(started.elapsed() < Duration::from_secs(2));
        drop(session);
        Ok(())
    }

    #[test]
    fn cancelled_projection_never_starts_an_unavailable_helper() -> Result<()> {
        use crate::media::{Decoder, ImageDetail, ImageKey, ImageProtocol};
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"/>"#;
        let image = Decoder::default()
            .render(
                svg,
                &ImageKey {
                    url: "https://example.org/diagram.svg".into(),
                    width: 1,
                    height: 1,
                    background: [0, 0, 0],
                    detail: Some(ImageDetail {
                        zoom: 1,
                        pan: [0, 0],
                        protocol: ImageProtocol::Overlay,
                        cell_pixels: (10, 20),
                        slice: None,
                    }),
                },
            )?
            .overlay
            .context("Prepared fixture PNG")?;
        let mut layer = Layer::new(Err("Missing optional helper".into()));
        layer.apply(Request {
            desired: Some(vec![Placement {
                id: "cancelled".into(),
                area: ratatui::layout::Rect::new(0, 0, 1, 1),
                image,
                crop: None,
            }]),
            valid: Arc::new(AtomicBool::new(false)),
            geometry: None,
        })?;
        assert!(layer.session.is_none());
        Ok(())
    }
}
