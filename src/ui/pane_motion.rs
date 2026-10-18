//! A captured pane floats above the workspace; only cell copies move per frame.

use ratatui::{
    Frame,
    buffer::Buffer,
    style::{Color, Style},
    widgets::{Block, Clear},
};
use unicode_width::UnicodeWidthStr;

use crate::app::{App, FloatingPane, Mode};

pub(super) fn draw(frame: &mut Frame<'_>, app: &App) {
    if !matches!(app.mode, Mode::Normal) {
        return;
    }
    let Some(picture) = app.floating_pane() else {
        super::chrome::draw_panel_move(frame, app);
        return;
    };
    let colors = &picture.cells[(picture.cells.area.x, picture.cells.area.y)];
    let surface = colors.bg;
    let region = app.pane_motion_region().unwrap_or(picture.slot);
    frame.render_widget(Clear, region);
    frame.render_widget(Block::new().style(Style::new().bg(colors.bg)), region);
    super::chrome::draw_panel_move(frame, app);
    for neighbor in app.neighboring_panes() {
        copy_picture(frame.buffer_mut(), &neighbor, None);
    }
    let held_surface = if app.panel_drag().is_some() {
        dim_workspace(frame.buffer_mut());
        Some(dim_color(surface))
    } else {
        None
    };
    copy_picture(frame.buffer_mut(), &picture, held_surface);
}

fn copy_picture(
    buffer: &mut Buffer,
    picture: &FloatingPane<'_>,
    surface: Option<Color>,
) {
    let visible = picture.area.intersection(buffer.area);
    let neutral =
        picture.cells[(picture.cells.area.x, picture.cells.area.y)].bg;
    for y in visible.y..visible.bottom() {
        for x in visible.x..visible.right() {
            let source = (
                picture.cells.area.x + x - picture.area.x,
                picture.cells.area.y + y - picture.area.y,
            );
            let cell = &mut buffer[(x, y)];
            *cell = picture.cells[source].clone();
            // A terminal cannot clip backgrounds to a curved glyph. Match
            // the dim canvas across the whole surface, keeping bright text
            // and the rounded outline without a rectangular plate or notch.
            if let Some(surface) = surface.filter(|_| cell.bg == neutral) {
                cell.set_bg(surface);
            }
            // A partial wide grapheme would wrap beyond the terminal edge.
            if cell.symbol().width() > usize::from(visible.right() - x) {
                cell.set_symbol(" ");
            }
        }
    }
}

fn dim_workspace(buffer: &mut Buffer) {
    // Fade the workspace before drawing the held pane with bright foregrounds.
    // A fresh workspace is drawn each frame, so this never compounds.
    for cell in &mut buffer.content {
        cell.set_fg(dim_color(cell.fg));
        cell.set_bg(dim_color(cell.bg));
    }
}

fn dim_color(color: Color) -> Color {
    match color {
        Color::Rgb(red, green, blue) => Color::Rgb(
            (u16::from(red) * 3 / 5) as u8,
            (u16::from(green) * 3 / 5) as u8,
            (u16::from(blue) * 3 / 5) as u8,
        ),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use ratatui::layout::Rect;

    use super::*;
    use crate::model::Pane;

    #[test]
    fn held_surface_has_no_corner_notches_and_occludes_background_content() {
        let source = pane_with_code();
        for surface in [None, Some(Color::Rgb(13, 13, 15))] {
            let mut target = Buffer::empty(Rect::new(0, 0, 8, 6));
            for cell in &mut target.content {
                cell.set_symbol("x").set_bg(Color::Rgb(7, 9, 11));
            }
            let picture = FloatingPane {
                pane: Pane::Reader,
                slot: source.area,
                area: Rect::new(2, 1, 4, 4),
                cells: &source,
            };

            copy_picture(&mut target, &picture, surface);

            for y in 0..4 {
                for x in 0..4 {
                    let mut expected = source[(x, y)].clone();
                    if expected.bg == Color::Rgb(23, 23, 26) {
                        expected.set_bg(surface.unwrap_or(expected.bg));
                    }
                    assert_eq!(target[(x + 2, y + 1)], expected);
                }
            }
            assert_eq!(target[(1, 1)].symbol(), "x");
            assert_eq!(target[(6, 1)].bg, Color::Rgb(7, 9, 11));
            assert_eq!(source[(0, 0)].bg, Color::Rgb(23, 23, 26));
        }
    }

    fn pane_with_code() -> Buffer {
        use ratatui::widgets::{BorderType, Widget};

        let mut source = Buffer::empty(Rect::new(0, 0, 4, 4));
        Block::bordered()
            .border_type(BorderType::Rounded)
            .style(Style::new().bg(Color::Rgb(23, 23, 26)).fg(Color::Yellow))
            .render(source.area, &mut source);
        source[(1, 1)]
            .set_symbol("c")
            .set_fg(Color::Cyan)
            .set_bg(Color::Rgb(37, 35, 33));
        source
    }

    #[test]
    fn clipped_wide_glyphs_never_wrap_into_the_following_terminal_row() {
        let mut source = Buffer::empty(Rect::new(0, 0, 3, 1));
        source[(1, 0)].set_symbol("界");
        for (column, expected) in [(2, "界"), (4, " ")] {
            let mut target = Buffer::empty(Rect::new(0, 0, 6, 2));
            target[(0, 1)].set_symbol("x");
            let picture = FloatingPane {
                pane: Pane::Reader,
                slot: source.area,
                area: Rect::new(column, 0, 3, 1),
                cells: &source,
            };

            copy_picture(&mut target, &picture, None);

            assert_eq!(target[(column + 1, 0)].symbol(), expected);
            assert_eq!(target[(0, 1)].symbol(), "x");
            assert_eq!(source[(1, 0)].symbol(), "界");
        }
    }
}
