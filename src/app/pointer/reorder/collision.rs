//! Leading-edge crossings select insertion slots independently of the grip.

use std::cmp::Ordering;

use ratatui::layout::Rect;

use crate::model::Pane;

pub(super) fn target(
    slots: &[(Pane, Rect)],
    origin: Rect,
    area: Rect,
    previous: Option<Pane>,
) -> Option<Pane> {
    let direction = area.x.cmp(&origin.x);
    if direction.is_eq() {
        return None;
    }
    let held_cells = u32::from(area.width) * u32::from(area.height);
    let candidates = slots.iter().filter(|(pane, slot)| {
        let smaller =
            held_cells.min(u32::from(slot.width) * u32::from(slot.height));
        // Enter at 25% of the smaller surface; retain until below 12.5%.
        let divisor = if previous == Some(*pane) { 8 } else { 4 };
        slot.x.cmp(&origin.x) == direction
            && smaller > 0
            && crossed_surface(area, *slot, direction)
                >= smaller.div_ceil(divisor)
    });
    let selected = if direction.is_lt() {
        candidates.min_by_key(|(_, slot)| slot.x)
    } else {
        candidates.max_by_key(|(_, slot)| slot.x)
    };
    selected.map(|(pane, _)| *pane)
}

fn crossed_surface(area: Rect, slot: Rect, direction: Ordering) -> u32 {
    let rows = area
        .bottom()
        .min(slot.bottom())
        .saturating_sub(area.y.max(slot.y));
    let columns = if direction.is_lt() {
        slot.right().saturating_sub(area.x).min(slot.width)
    } else {
        area.right().saturating_sub(slot.x).min(slot.width)
    };
    // Keep crossed slots eligible after passing them. Moving partly offscreen
    // must not undo an insertion already established by the leading edge.
    u32::from(rows) * u32::from(columns)
}

#[cfg(test)]
mod tests;
