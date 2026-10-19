//! Persistent X11 backgrounds survive movement, clipping and expose events.
//!
//! The presentation worker owns this connection and its child windows. Source
//! pixels upload only on image changes. Scrolling changes position and Shape
//! bounds without unmapping a window or asking another process to redraw it.
//! Closing the connection releases every window, pixmap and graphics context.

mod surface;

use std::sync::atomic::Ordering;
use std::{collections::BTreeMap, sync::Arc};

use anyhow::{Context, Result};
use x11rb::{
    connection::Connection,
    image::PixelLayout,
    protocol::{shape::ConnectionExt as _, xproto::*},
    rust_connection::RustConnection,
};

use super::{Geometry, GridOrigin, Placement, Request};
use surface::Surface;

pub(super) struct Layer {
    display: String,
    parent: u32,
    origin: GridOrigin,
    geometry: Option<Geometry>,
    canvas: Option<Canvas>,
}

struct Canvas {
    display: Display,
    surfaces: BTreeMap<String, Surface>,
}

struct Display {
    connection: Arc<RustConnection>,
    parent: u32,
    gc: u32,
    depth: u8,
    visual: u32,
    colormap: u32,
    layout: PixelLayout,
    opaque: u32,
    geometry: Geometry,
    origin: [i32; 2],
}

impl Layer {
    pub(super) fn new(
        display: String,
        parent: u32,
        origin: GridOrigin,
    ) -> Self {
        Self {
            display,
            parent,
            origin,
            geometry: None,
            canvas: None,
        }
    }

    pub(super) fn apply(&mut self, request: Request) -> Result<()> {
        if !request.valid.load(Ordering::Acquire) {
            return Ok(());
        }
        let Some(desired) = request.desired else {
            self.canvas = None;
            return Ok(());
        };
        super::validate(&desired)?;
        if self.geometry != request.geometry {
            self.canvas = None;
            self.geometry = request.geometry;
        }
        if self.canvas.is_none() {
            let geometry =
                request.geometry.context("Missing terminal geometry")?;
            self.canvas = Some(Canvas {
                display: Display::connect(
                    &self.display,
                    self.parent,
                    geometry,
                    self.origin,
                )?,
                surfaces: BTreeMap::new(),
            });
        }
        let result = self
            .canvas
            .as_mut()
            .context("Missing X11 canvas")?
            .sync(&desired, &request.valid);
        if result.is_err() || !request.valid.load(Ordering::Acquire) {
            self.canvas = None;
        }
        result
    }
}

impl Canvas {
    fn sync(
        &mut self,
        desired: &[Placement],
        valid: &std::sync::atomic::AtomicBool,
    ) -> Result<()> {
        for placement in desired {
            if !valid.load(Ordering::Acquire) {
                return Ok(());
            }
            let unchanged = self
                .surfaces
                .get(&placement.id)
                .is_some_and(|surface| surface.matches(placement));
            if !unchanged {
                let mut surface = Surface::new(&self.display, placement)?;
                surface.place(&self.display, placement)?;
                self.display
                    .connection
                    .map_window(surface.window())?
                    .check()?;
                // Map a fully painted replacement before retiring its old window.
                self.surfaces.insert(placement.id.clone(), surface);
            } else if let Some(surface) = self.surfaces.get_mut(&placement.id) {
                surface.place(&self.display, placement)?;
            }
        }
        self.surfaces
            .retain(|id, _| desired.iter().any(|image| &image.id == id));
        self.display.connection.flush()?;
        self.display.connection.get_input_focus()?.reply()?;
        Ok(())
    }
}

impl Display {
    fn connect(
        address: &str,
        parent: u32,
        geometry: Geometry,
        grid_origin: GridOrigin,
    ) -> Result<Self> {
        let (connection, _) = RustConnection::connect(Some(address))?;
        let connection = Arc::new(connection);
        let parent_geometry = connection.get_geometry(parent)?.reply()?;
        let version = connection.shape_query_version()?.reply()?;
        anyhow::ensure!(
            version.major_version > 1 || version.minor_version >= 1,
            "X11 Shape 1.1 is required for mouse passthrough"
        );
        let (depth, visual, colormap) =
            root_format(&connection, parent_geometry.root)?;
        let layout = PixelLayout::from_visual_type(visual)?;
        let opaque = if depth == 32 {
            !(visual.red_mask | visual.green_mask | visual.blue_mask)
        } else {
            0
        };
        let gc = connection.generate_id()?;
        connection
            .create_gc(gc, parent_geometry.root, &CreateGCAux::new())?
            .check()?;
        let origin = origin(
            grid_origin,
            (parent_geometry.width, parent_geometry.height),
            geometry,
        );
        Ok(Self {
            connection,
            parent,
            gc,
            depth,
            visual: visual.visual_id,
            colormap,
            layout,
            opaque,
            geometry,
            origin,
        })
    }
}

fn root_format(
    connection: &RustConnection,
    root: u32,
) -> Result<(u8, Visualtype, u32)> {
    let screen = connection
        .setup()
        .roots
        .iter()
        .find(|screen| screen.root == root)
        .context("Unknown terminal screen")?;
    let visual = screen
        .allowed_depths
        .iter()
        .flat_map(|depth| &depth.visuals)
        .find(|visual| visual.visual_id == screen.root_visual)
        .context("Missing root visual")?;
    Ok((screen.root_depth, *visual, screen.default_colormap))
}

fn origin(
    policy: GridOrigin,
    window: (u16, u16),
    geometry: Geometry,
) -> [i32; 2] {
    match policy {
        GridOrigin::Fixed(padding) => padding.map(i32::from),
        GridOrigin::Centered => [
            padding(window.0, geometry.cells.0, geometry.pixels.0),
            padding(window.1, geometry.cells.1, geometry.pixels.1),
        ],
    }
}

fn padding(pixels: u16, cells: u16, cell_pixels: u16) -> i32 {
    (i32::from(pixels) - i32::from(cells) * i32::from(cell_pixels)).max(0) / 2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_padding_never_centers_fractional_cell_remainders() {
        let geometry = Geometry {
            cells: (129, 41),
            pixels: (10, 19),
        };
        assert_eq!(
            origin(GridOrigin::Fixed([0, 0]), (1296, 792), geometry),
            [0, 0]
        );
        assert_eq!(
            origin(GridOrigin::Fixed([8, 12]), (1296, 792), geometry),
            [8, 12]
        );
        assert_eq!(origin(GridOrigin::Centered, (1296, 792), geometry), [3, 6]);
        assert_eq!(padding(100, 144, 9), 0);
    }
}
