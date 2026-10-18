//! Time and snapshot limits are qualified without sleeping or a terminal.

use super::*;

#[test]
fn snapshot_rejects_empty_outside_and_oversized_regions() {
    let mut frame = Buffer::empty(Rect::new(0, 0, 500, 100));
    frame[(3, 4)].set_symbol("é");

    assert!(snapshot(&frame, Rect::new(0, 0, 0, 1)).is_none());
    assert!(snapshot(&frame, Rect::new(499, 99, 2, 2)).is_none());
    assert!(snapshot(&frame, Rect::new(0, 0, 500, 50)).is_none());
    let small = snapshot(&frame, Rect::new(3, 4, 2, 2));
    assert_eq!(
        small.as_ref().map(|buffer| buffer[(3, 4)].symbol()),
        Some("é")
    );
    assert_eq!(small.map(|buffer| buffer.content.len()), Some(4));
}

#[test]
fn easing_has_exact_endpoints_a_known_midpoint_and_bounded_monotonic_motion() {
    assert_eq!(eased_coordinate(0, 80, Duration::ZERO), 0);
    assert_eq!(eased_coordinate(0, 80, Duration::from_millis(80)), 70);
    assert_eq!(eased_coordinate(80, 0, Duration::from_millis(80)), 10);
    assert_eq!(eased_coordinate(0, 80, Duration::from_millis(160)), 80);
    assert_eq!(eased_coordinate(80, 0, Duration::from_secs(5)), 0);
    let mut previous = 0;
    for millis in 0..=160 {
        let position = eased_coordinate(0, 80, Duration::from_millis(millis));
        assert!((previous..=80).contains(&position));
        previous = position;
    }
}

#[test]
fn stationary_pictures_can_anchor_neighbors_but_reflow_requires_real_content() {
    let from = Rect::new(0, 2, 20, 30);
    let picture = Buffer::empty(from);
    assert!(
        PaneFlight::start(Pane::Reader, picture.clone(), from, from).is_some()
    );
    let reflowed = Rect::new(20, 2, 30, 30);
    assert!(PaneFlight::start(Pane::Reader, picture, from, reflowed).is_none());
}

#[test]
fn retargeting_a_slide_preserves_its_current_position_and_does_not_restart_same_goal()
 {
    let now = Instant::now();
    let mut slide = Slide::new(Rect::new(0, 2, 20, 30));
    slide.retarget(Rect::new(80, 2, 20, 30), now);
    assert!(slide.tick(now + Duration::from_millis(80)));
    assert_eq!(slide.current.x, 70);

    slide.retarget(Rect::new(0, 2, 20, 30), now + Duration::from_millis(80));

    assert_eq!(slide.current.x, 70);
    slide.retarget(Rect::new(0, 2, 20, 30), now + Duration::from_millis(120));
    assert!(slide.tick(now + Duration::from_millis(240)));
    assert_eq!(slide.current.x, 0);
    assert!(!slide.active());
    assert!(!slide.tick(now + Duration::from_secs(4)));
}

#[test]
fn neighbor_capture_rejects_unbounded_workspace_memory() {
    let frame = Buffer::empty(Rect::new(0, 0, 600, 100));
    let slots = [
        (Pane::Library, Rect::new(0, 0, 100, 100)),
        (Pane::Reader, Rect::new(100, 0, 500, 100)),
    ];
    assert!(Neighbors::capture(&frame, &slots, Pane::Library).is_none());
}
