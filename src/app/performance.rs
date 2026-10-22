//! Opt-in, bounded processing samples; no telemetry or physical-latency claims.

use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

use anyhow::{Result, bail};

use super::{Action, App};

/// An application processing boundary measured only when diagnostics are enabled.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Operation {
    /// Dispatch of one input event or coalesced burst.
    Input,
    /// Layout, draw and terminal flush; excludes compositor presentation.
    Frame,
    /// Article navigation from admission to ready content.
    Article,
    /// Selector query and result delivery.
    Search,
    /// Selector preview admission and delivery.
    Preview,
    /// Ordered personal write, including queue wait and result delivery.
    Save,
}

#[derive(Default)]
pub(super) struct Performance {
    enabled: bool,
    samples: BTreeMap<Operation, std::collections::VecDeque<u64>>,
    started: BTreeMap<Operation, Instant>,
}

impl Performance {
    pub(super) fn begin(&mut self, operation: Operation) {
        if self.enabled {
            self.started.insert(operation, Instant::now());
        }
    }

    pub(super) fn finish(&mut self, operation: Operation) {
        if let Some(started) = self.started.remove(&operation) {
            self.record(operation, started.elapsed());
        }
    }

    pub(super) fn record(&mut self, operation: Operation, elapsed: Duration) {
        if !self.enabled {
            return;
        }
        let samples = self.samples.entry(operation).or_default();
        if samples.len() == 512 {
            samples.pop_front();
        }
        samples.push_back(elapsed.as_micros().min(u64::MAX as u128) as u64);
    }

    fn report(&self) -> String {
        let mut text = format!(
            "Diagnostics: {}\n\nOperation       n      p50      p95      p99      max\n",
            if self.enabled { "enabled" } else { "disabled" }
        );
        for (operation, samples) in &self.samples {
            let mut sorted = samples.iter().copied().collect::<Vec<_>>();
            sorted.sort_unstable();
            let percentile = |percent: usize| {
                let index = (sorted.len() * percent).div_ceil(100) - 1;
                sorted[index] as f64 / 1000.0
            };
            text.push_str(&format!(
                "{operation:<12?} {:>4} {:>8.3} {:>8.3} {:>8.3} {:>8.3}\n",
                sorted.len(),
                percentile(50),
                percentile(95),
                percentile(99),
                percentile(100)
            ));
        }
        text.push_str("\nMilliseconds; last 512 samples per operation.\n");
        text.push_str("These are processing/delivery times, not physical display latency.\n");
        text.push_str(
            "No samples leave this process. :performance on|off|reset\n",
        );
        text
    }
}

impl App {
    /// Begin an opt-in application measurement; disabled diagnostics do no timing.
    pub fn begin_measurement(&mut self, operation: Operation) {
        self.performance.begin(operation);
    }

    /// Finish the matching application measurement, if one was started.
    pub fn finish_measurement(&mut self, operation: Operation) {
        self.performance.finish(operation);
    }

    pub(super) fn performance_command(
        &mut self,
        argument: &str,
    ) -> Result<Action> {
        match argument {
            "on" => self.performance.enabled = true,
            "off" => {
                self.performance.enabled = false;
                self.performance.started.clear();
            }
            "reset" => {
                self.performance.samples.clear();
                self.performance.started.clear();
            }
            "" | "status" => {}
            _ => bail!("Use :performance on|off|reset|status"),
        }
        if let Some(writer) = &mut self.personal_writes {
            writer.set_diagnostics(self.performance.enabled);
        }
        let mut report = self.performance.report();
        report.push_str(&format!(
            concat!(
                "\nPending: navigation={} selector={} notes={} ",
                "session={} images={} web={} maintenance={}\n"
            ),
            self.navigation_loading,
            self.picker_pending(),
            self.notes_pending(),
            self.session_pending(),
            self.images_pending(),
            self.web_pending(),
            self.maintenance_pending(),
        ));
        self.show_document(
            "Performance · processing times · Esc closes",
            &report,
            None,
        );
        Ok(Action::None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_are_opt_in_bounded_and_use_nearest_rank_percentiles() {
        let mut performance = Performance::default();
        performance.record(Operation::Input, Duration::from_secs(1));
        assert!(performance.samples.is_empty());
        performance.enabled = true;
        for value in 0..612 {
            performance.record(Operation::Input, Duration::from_millis(value));
        }
        assert_eq!(performance.samples[&Operation::Input].len(), 512);
        let report = performance.report();
        for value in ["355.000", "586.000", "606.000", "611.000"] {
            assert!(report.contains(value), "Missing percentile {value}");
        }
    }
}
