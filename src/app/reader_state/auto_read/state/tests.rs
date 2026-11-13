//! Coverage, inactivity and explicit intent stay independent of wall-clock time.
use super::*;
use crate::model::ContentSource;

fn state(length: usize) -> State {
    let mut state = State::default();
    state.source(
        ReaderIdentity {
            article: 1,
            source: ContentSource::Feed,
            revision: 0,
        },
        length,
        AutoReadSettings {
            min_active_seconds: 1,
            ..AutoReadSettings::default()
        },
    );
    state
}

#[test]
fn overlapping_adjacent_and_clipped_ranges_only_count_once() {
    let mut state = state(100);
    for range in [20..40, 30..60, 0..20, 90..200, 200..300, 60..90] {
        state.record(range);
    }
    assert_eq!(state.covered, 100);
    assert_eq!(state.ranges, vec![0..100]);
}

#[test]
fn fragmented_coverage_is_bounded_and_can_still_merge_at_capacity() {
    let mut state = state(5000);
    for start in (0..3000).step_by(2) {
        state.record(start..start + 1);
    }
    assert_eq!(state.ranges.len(), 1024);
    assert_eq!(state.covered, 1024);
    state.record(0..5000);
    assert_eq!(state.ranges, vec![0..5000]);
    assert_eq!(state.covered, 5000);
}

#[test]
fn pauses_and_long_scheduler_stalls_do_not_count_as_active_time() {
    let mut state = state(100);
    state.record(0..100);
    let now = Instant::now();
    assert!(!state.tick(now));
    assert!(!state.tick(now + Duration::from_millis(500)));
    state.pause();
    assert!(!state.tick(now + Duration::from_secs(60)));
    assert!(!state.tick(now + Duration::from_secs(120)));
    assert!(state.tick(now + Duration::from_millis(120_500)));
    state.committed_intent();
    assert!(!state.tick(now + Duration::from_secs(121)));
}

#[test]
fn source_changes_reset_progress_and_manual_intent_survives_reflow() {
    let mut state = state(100);
    state.record(0..100);
    state.block(1);
    state.reset_text();
    let original = ReaderIdentity {
        article: 1,
        source: ContentSource::Full,
        revision: 2,
    };
    state.source(original, 200, AutoReadSettings::default());
    assert_eq!(state.covered, 0);
    assert_eq!(state.blocked, Some(1));
    state.source(
        ReaderIdentity {
            article: 2,
            ..original
        },
        100,
        AutoReadSettings::default(),
    );
    assert_eq!(state.blocked, None);
}

#[test]
fn policy_changes_restart_time_and_coverage() {
    let mut state = state(100);
    state.record(0..100);
    let now = Instant::now();
    state.tick(now);
    let identity = ReaderIdentity {
        article: 1,
        source: ContentSource::Feed,
        revision: 0,
    };
    state.source(identity, 100, AutoReadSettings::default());
    assert_eq!(state.covered, 0);
    assert_eq!(state.active, Duration::ZERO);
    assert!(!state.tick(now + Duration::from_secs(60)));
}
