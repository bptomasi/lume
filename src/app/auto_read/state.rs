//! Monotonic time and bounded interval unions never mistake a jump for reading.
use crate::{app::ReaderIdentity, model::AutoReadSettings};
use std::{
    ops::Range,
    time::{Duration, Instant},
};

#[cfg(test)]
mod tests;

#[derive(Default)]
pub(super) struct State {
    pub(super) identity: Option<ReaderIdentity>,
    pub(super) length: usize,
    pub(super) view: Option<(usize, usize, usize, usize)>,
    ranges: Vec<Range<usize>>,
    covered: usize,
    active: Duration,
    last: Option<Instant>,
    blocked: Option<i64>,
    done: bool,
    policy: AutoReadSettings,
}

impl State {
    pub(super) fn source(
        &mut self,
        identity: ReaderIdentity,
        length: usize,
        policy: AutoReadSettings,
    ) {
        if self.identity == Some(identity)
            && self.length == length
            && self.policy == policy
        {
            return;
        }
        let blocked = self.blocked.filter(|id| *id == identity.article);
        *self = Self {
            identity: Some(identity),
            length,
            policy,
            blocked,
            ..Self::default()
        };
    }

    pub(super) fn reset_text(&mut self) {
        self.identity = None;
        self.pause();
    }

    pub(super) fn pause(&mut self) {
        self.last = None;
    }

    pub(super) fn block(&mut self, article: i64) {
        self.blocked = Some(article);
        self.pause();
    }

    pub(super) fn record(&mut self, mut range: Range<usize>) {
        range.end = range.end.min(self.length);
        if range.start >= range.end {
            return;
        }
        let first = self.ranges.partition_point(|old| old.end < range.start);
        let mut end = first;
        while let Some(old) = self.ranges.get(end) {
            if old.start > range.end {
                break;
            }
            range.start = range.start.min(old.start);
            range.end = range.end.max(old.end);
            end += 1;
        }
        if first == end && self.ranges.len() >= 1024 {
            return;
        }
        for old in &self.ranges[first..end] {
            self.covered -= old.len();
        }
        self.covered += range.len();
        self.ranges.splice(first..end, [range]);
    }

    pub(super) fn tick(&mut self, now: Instant) -> bool {
        if let Some(last) = self.last.replace(now) {
            let elapsed = now.saturating_duration_since(last);
            // Suspended processes or long event-loop stalls are not active reading.
            if elapsed <= Duration::from_secs(1) {
                self.active += elapsed;
            }
        }
        !self.done
            && self.length > 0
            && self.policy.enabled
            && self
                .identity
                .is_some_and(|identity| self.blocked != Some(identity.article))
            && self.active
                >= Duration::from_secs(u64::from(
                    self.policy.min_active_seconds,
                ))
            && self.covered as u128 * 100
                >= self.length as u128
                    * u128::from(self.policy.min_coverage_percent)
    }

    pub(super) fn committed_intent(&mut self) {
        self.done = true;
    }
}
