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
    /// Image source decode/orientation or vector parsing.
    ImageDecode,
    /// Image matte, crop, resampling or vector rasterization.
    ImageFit,
    /// RGB ownership and palette compaction.
    ImagePixels,
    /// Exact terminal-color detection.
    ImageExact,
    /// Deduplication and Oklab preparation.
    ImageColors,
    /// Wu histogram, palette and color map construction.
    ImagePalette,
    /// Indexed mapping and error diffusion.
    ImageDither,
    /// Sixel protocol encoding.
    ImageEncode,
    /// Image work including cache/HTTP and decoder wait, excluding UI queue wait.
    ImageWork,
}

#[derive(Default)]
pub(super) struct Performance {
    enabled: bool,
    samples: BTreeMap<Operation, std::collections::VecDeque<u64>>,
    started: BTreeMap<Operation, Instant>,
}

impl Performance {
    pub(super) fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub(super) fn record_image(
        &mut self,
        sample: &crate::media::profiling::Sample,
    ) {
        use crate::media::profiling::Step;
        for (step, operation) in [
            (Step::Decode, Operation::ImageDecode),
            (Step::Resize, Operation::ImageFit),
            (Step::Pixels, Operation::ImagePixels),
            (Step::Exact, Operation::ImageExact),
            (Step::Colors, Operation::ImageColors),
            (Step::Quantize, Operation::ImagePalette),
            (Step::Dither, Operation::ImageDither),
            (Step::Encode, Operation::ImageEncode),
        ] {
            if sample.calls[step as usize] > 0 {
                self.record(
                    operation,
                    Duration::from_nanos(sample.stages_ns[step as usize]),
                );
            }
        }
        self.record(
            Operation::ImageWork,
            Duration::from_nanos(sample.total_ns),
        );
    }

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

    #[test]
    fn image_profiles_skip_absent_stages_and_do_not_record_when_disabled() {
        use crate::media::profiling::{Sample, Step};
        let mut sample = Sample {
            total_ns: 9_000_000,
            ..Sample::default()
        };
        sample.calls[Step::Encode as usize] = 12;
        sample.stages_ns[Step::Encode as usize] = 4_000_000;
        let mut performance = Performance::default();
        performance.record_image(&sample);
        assert!(performance.samples.is_empty());
        performance.enabled = true;
        performance.record_image(&sample);
        assert_eq!(performance.samples[&Operation::ImageEncode].len(), 1);
        assert_eq!(performance.samples[&Operation::ImageEncode][0], 4_000);
        assert!(!performance.samples.contains_key(&Operation::ImageDecode));
        assert!(performance.report().contains("ImageWork"));
    }
}
