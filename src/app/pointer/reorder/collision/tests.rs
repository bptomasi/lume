//! Independent cell coordinates qualify entry, retention and viewport edges.

use super::*;

const SLOTS: [(Pane, Rect); 4] = [
    (Pane::Library, Rect::new(0, 2, 20, 40)),
    (Pane::Articles, Rect::new(20, 2, 40, 40)),
    (Pane::Reader, Rect::new(60, 2, 100, 40)),
    (Pane::Annotations, Rect::new(160, 2, 40, 40)),
];

#[test]
fn left_overlap_has_distinct_entry_and_retention_boundaries() {
    let origin = SLOTS[2].1;
    assert_eq!(
        target(&SLOTS, origin, Rect::new(51, 2, 100, 40), None),
        None
    );
    assert_eq!(
        target(&SLOTS, origin, Rect::new(50, 2, 100, 40), None),
        Some(Pane::Articles)
    );
    assert_eq!(
        target(
            &SLOTS,
            origin,
            Rect::new(54, 2, 100, 40),
            Some(Pane::Articles)
        ),
        Some(Pane::Articles)
    );
    assert_eq!(
        target(
            &SLOTS,
            origin,
            Rect::new(56, 2, 100, 40),
            Some(Pane::Articles)
        ),
        None
    );
    assert_eq!(target(&SLOTS, origin, origin, Some(Pane::Articles)), None);
}

#[test]
fn right_overlap_starts_early_and_remains_stable_at_the_terminal_edge() {
    let origin = SLOTS[2].1;
    assert_eq!(
        target(&SLOTS, origin, Rect::new(69, 2, 100, 40), None),
        None
    );
    assert_eq!(
        target(&SLOTS, origin, Rect::new(70, 2, 100, 40), None),
        Some(Pane::Annotations)
    );
    assert_eq!(
        target(
            &SLOTS,
            origin,
            Rect::new(198, 2, 100, 40),
            Some(Pane::Annotations)
        ),
        Some(Pane::Annotations)
    );
}

#[test]
fn farthest_crossed_slot_wins_and_vertical_separation_cancels() {
    let origin = SLOTS[2].1;
    assert_eq!(
        target(&SLOTS, origin, Rect::new(0, 2, 100, 40), None),
        Some(Pane::Library)
    );
    assert_eq!(
        target(
            &SLOTS,
            origin,
            Rect::new(0, 43, 100, 40),
            Some(Pane::Library)
        ),
        None
    );
    assert_eq!(
        target(&SLOTS, origin, Rect::new(50, 12, 100, 40), None),
        None
    );
    assert_eq!(
        target(&SLOTS, origin, Rect::new(44, 12, 100, 40), None),
        Some(Pane::Articles)
    );
}
