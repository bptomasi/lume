//! A server pixmap is both the initial frame and every later exposed background.

use std::{ops::Range, sync::Arc};

use anyhow::{Context, Result, ensure};
use ratatui::layout::Rect;
use x11rb::{
    connection::Connection,
    image::Image,
    protocol::{
        shape::{self, ConnectionExt as _},
        xproto::*,
    },
    rust_connection::RustConnection,
    wrapper::ConnectionExt as _,
};

use crate::{media::PreparedPng, overlay::Placement};

use super::Display;

pub(super) struct Surface {
    connection: Arc<RustConnection>,
    window: u32,
    pixmap: u32,
    source: Arc<PreparedPng>,
    size: (u16, u16),
    position: Option<(Rect, Option<Range<u32>>)>,
}

impl Surface {
    pub(super) fn new(
        display: &Display,
        placement: &Placement,
    ) -> Result<Self> {
        let image = pixels(display, &placement.image)?;
        let pixmap = upload(display, &image)?;
        let window = create_window(
            display,
            placement,
            pixmap,
            (image.width(), image.height()),
        )?;
        Ok(Self {
            connection: display.connection.clone(),
            window,
            pixmap,
            source: placement.image.clone(),
            size: (image.width(), image.height()),
            position: None,
        })
    }

    pub(super) fn matches(&self, placement: &Placement) -> bool {
        Arc::ptr_eq(&self.source, &placement.image)
    }

    pub(super) fn window(&self) -> u32 {
        self.window
    }

    pub(super) fn place(
        &mut self,
        display: &Display,
        placement: &Placement,
    ) -> Result<()> {
        let position = (placement.area, placement.crop.clone());
        if self.position.as_ref() == Some(&position) {
            return Ok(());
        }
        let rows = placement.crop.clone().unwrap_or(0..u32::from(self.size.1));
        let (x, y, clip) = geometry(
            display.origin,
            display.geometry.pixels,
            placement.area,
            rows,
            self.size,
        )?;
        let shape = self.connection.shape_rectangles(
            shape::SO::SET,
            shape::SK::BOUNDING,
            ClipOrdering::UNSORTED,
            self.window,
            0,
            0,
            &[clip],
        )?;
        let movement = self.connection.configure_window(
            self.window,
            &ConfigureWindowAux::new().x(x).y(y),
        )?;
        // Flush shape and movement together; neither operation unmaps the image.
        self.connection.flush()?;
        shape.check()?;
        movement.check()?;
        self.position = Some(position);
        Ok(())
    }
}

fn upload(display: &Display, image: &Image<'_>) -> Result<u32> {
    let connection = &display.connection;
    let pixmap = connection.generate_id()?;
    connection
        .create_pixmap(
            display.depth,
            pixmap,
            display.parent,
            image.width(),
            image.height(),
        )?
        .check()?;
    for cookie in image.put(connection.as_ref(), pixmap, display.gc, 0, 0)? {
        cookie.check()?;
    }
    Ok(pixmap)
}

fn create_window(
    display: &Display,
    placement: &Placement,
    pixmap: u32,
    size: (u16, u16),
) -> Result<u32> {
    let connection = &display.connection;
    let window = connection.generate_id()?;
    let attributes = CreateWindowAux::new()
        .background_pixmap(pixmap)
        .border_pixel(0)
        .colormap(display.colormap);
    connection
        .create_window(
            display.depth,
            window,
            display.parent,
            0,
            0,
            size.0,
            size.1,
            0,
            WindowClass::INPUT_OUTPUT,
            display.visual,
            &attributes,
        )?
        .check()?;
    connection.change_property8(
        PropMode::REPLACE,
        window,
        AtomEnum::WM_NAME,
        AtomEnum::STRING,
        placement.id.as_bytes(),
    )?;
    // Input goes directly to the terminal, including image clicks and hover.
    connection
        .shape_rectangles(
            shape::SO::SET,
            shape::SK::INPUT,
            ClipOrdering::UNSORTED,
            window,
            0,
            0,
            &[],
        )?
        .check()?;
    Ok(window)
}

fn pixels(display: &Display, source: &PreparedPng) -> Result<Image<'static>> {
    let pixels = image::open(source.path())?.to_rgb8();
    ensure!(
        u64::from(pixels.width()) * u64::from(pixels.height()) <= 4_000_000,
        "Prepared X11 image exceeds four million pixels"
    );
    let width = u16::try_from(pixels.width())?;
    let height = u16::try_from(pixels.height())?;
    let mut image = Image::allocate_native(
        width,
        height,
        display.depth,
        display.connection.setup(),
    )?;
    for (x, y, pixel) in pixels.enumerate_pixels() {
        let [red, green, blue] = pixel.0.map(|value| u16::from(value) * 257);
        let color = display.layout.encode((red, green, blue));
        image.put_pixel(x as u16, y as u16, color | display.opaque);
    }
    Ok(image)
}

fn geometry(
    origin: [i32; 2],
    cell: (u16, u16),
    area: Rect,
    rows: Range<u32>,
    size: (u16, u16),
) -> Result<(i32, i32, Rectangle)> {
    let end = rows.end.min(u32::from(size.1));
    ensure!(rows.start < end, "X11 crop has no visible pixels");
    let x = origin[0] + i32::from(area.x) * i32::from(cell.0);
    let y = origin[1] + i32::from(area.y) * i32::from(cell.1)
        - i32::try_from(rows.start)?;
    i16::try_from(x).context("X11 image exceeds horizontal coordinates")?;
    i16::try_from(y).context("X11 image exceeds vertical coordinates")?;
    let clip = Rectangle {
        x: 0,
        y: i16::try_from(rows.start)?,
        width: size.0.min(area.width.saturating_mul(cell.0)),
        height: u16::try_from(end - rows.start)?
            .min(area.height.saturating_mul(cell.1)),
    };
    ensure!(clip.width > 0 && clip.height > 0, "Empty X11 clip");
    Ok((x, y, clip))
}

impl Drop for Surface {
    fn drop(&mut self) {
        let _ = self.connection.destroy_window(self.window);
        let _ = self.connection.free_pixmap(self.pixmap);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moving_and_clipping_preserves_source_pixel_coordinates() -> Result<()> {
        let (x, y, clip) = geometry(
            [2, 3],
            (9, 18),
            Rect::new(32, 7, 80, 9),
            54..216,
            (718, 216),
        )?;
        assert_eq!((x, y), (290, 75));
        assert_eq!(
            (clip.x, clip.y, clip.width, clip.height),
            (0, 54, 718, 162)
        );
        assert_eq!(y + i32::from(clip.y), 3 + 7 * 18);

        let (_, y, clip) = geometry(
            [2, 3],
            (9, 18),
            Rect::new(32, 7, 80, 5),
            0..90,
            (718, 216),
        )?;
        assert_eq!(y, 129);
        assert_eq!((clip.y, clip.width, clip.height), (0, 718, 90));
        Ok(())
    }

    #[test]
    fn empty_or_unrepresentable_bounds_are_rejected() {
        let area = Rect::new(32, 7, 80, 9);
        assert!(geometry([0, 0], (9, 18), area, 216..230, (718, 216)).is_err());
        assert!(
            geometry([0, 0], (9, 18), Rect::default(), 0..90, (718, 216))
                .is_err()
        );
        assert!(
            geometry(
                [0, 0],
                (64, 128),
                Rect::new(u16::MAX, 0, 1, 1),
                0..90,
                (64, 128)
            )
            .is_err()
        );
    }
}
