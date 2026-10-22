//! Split resizing consumes the same boundaries emitted during layout.

use crossterm::event::MouseEvent;
use ratatui::layout::Position;

use crate::ui;

use super::{App, Gesture};

impl App {
    pub(super) fn begin_split_resize(
        &mut self,
        areas: &ui::Areas,
        event: MouseEvent,
    ) -> bool {
        let Some(tree) = &self.workspace.layout else {
            return false;
        };
        let position = Position::new(event.column, event.row);
        let Some(boundary) = areas
            .boundaries
            .iter()
            .rev()
            .find(|boundary| on_seam(boundary, position))
        else {
            return false;
        };
        self.pointer.gesture = Some(Gesture::SplitResize {
            boundary: boundary.clone(),
            grab: position,
            before: tree.clone(),
        });
        true
    }

    pub(super) fn drag_split(
        &mut self,
        boundary: &ui::SplitBoundary,
        grab: Position,
        event: MouseEvent,
    ) {
        let delta = if boundary.horizontal {
            i32::from(event.column) - i32::from(grab.x)
        } else {
            i32::from(event.row) - i32::from(grab.y)
        };
        let share = boundary.dragged_percent(delta);
        let Some(tree) = &mut self.workspace.layout else {
            return;
        };
        if !tree.resize_path(&boundary.path, share) {
            return;
        }
        self.resize_workspace(self.screen);
        self.status = "Resizing split · release saves · Esc cancels".into();
    }
}

fn on_seam(boundary: &ui::SplitBoundary, position: Position) -> bool {
    let area = boundary.first.union(boundary.second);
    if boundary.horizontal {
        return position.x.abs_diff(boundary.second.x) <= 1
            && position.y > area.y
            && position.y + 1 < area.bottom();
    }
    position.y == boundary.first.bottom().saturating_sub(1)
        && position.x > area.x
        && position.x + 1 < area.right()
}
