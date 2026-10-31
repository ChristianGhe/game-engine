//! Budgets from `budgets.toml` (a protected oracle, spec 6.13). Only
//! runner-independent quantities are budgeted here; wall-clock budgets need
//! real hardware. `[sim]` holds the simulation in every mode; `[gpu]` holds
//! each frame `shot` and `video` render. Both tables are optional, and a key
//! the probe does not know is an error.

use eng_gpu::FrameCounters;
use eng_schema_json::{Authored, LoadError, load_toml};
use serde::Deserialize;
use serde::de::IgnoredAny;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BudgetFile {
    /// Checked by `load_toml`.
    #[serde(rename = "version")]
    _version: u32,
    #[serde(default)]
    pub sim: SimBudgets,
    #[serde(default)]
    pub gpu: GpuBudgets,
    /// Read by `cargo xtask`, not by the probe.
    #[serde(default, rename = "ci")]
    _ci: Option<IgnoredAny>,
}

impl Authored for BudgetFile {
    const KIND: &'static str = "budgets file";
    const VERSION: u32 = 1;
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SimBudgets {
    /// Largest canonical world encoding, in bytes.
    pub max_state_bytes: Option<u64>,
    pub max_entities: Option<u64>,
}

/// Per-frame limits on what a frame asks of the GPU ([`FrameCounters`]),
/// the same on every adapter.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GpuBudgets {
    pub max_draws_per_frame: Option<u64>,
    pub max_pipeline_switches_per_frame: Option<u64>,
    pub max_bind_group_switches_per_frame: Option<u64>,
    pub max_upload_bytes_per_frame: Option<u64>,
}

/// Peak usage against the limits.
#[derive(Debug)]
pub(crate) struct BudgetTracker {
    limits: SimBudgets,
    gpu: GpuBudgets,
    pub peak_state_bytes: u64,
    pub peak_entities: u64,
    /// The largest of each counter over the frames rendered, if any were.
    pub peak_frame: Option<FrameCounters>,
    pub exceeded: Option<String>,
}

impl BudgetTracker {
    pub fn unlimited() -> Self {
        Self::new(SimBudgets::default(), GpuBudgets::default())
    }

    fn new(limits: SimBudgets, gpu: GpuBudgets) -> Self {
        Self {
            limits,
            gpu,
            peak_state_bytes: 0,
            peak_entities: 0,
            peak_frame: None,
            exceeded: None,
        }
    }

    pub fn load(text: &str, origin: &str) -> Result<Self, LoadError> {
        let file: BudgetFile = load_toml(text, origin)?;
        Ok(Self::new(file.sim, file.gpu))
    }

    /// Records one tick's usage. Returns false once a budget is exceeded.
    pub fn observe(&mut self, tick: u64, state_bytes: u64, entities: u64) -> bool {
        self.peak_state_bytes = self.peak_state_bytes.max(state_bytes);
        self.peak_entities = self.peak_entities.max(entities);
        let checks = [
            ("max_state_bytes", self.limits.max_state_bytes, state_bytes),
            ("max_entities", self.limits.max_entities, entities),
        ];
        self.check(tick, checks)
    }

    /// Records one rendered frame's counters, at the tick it shows. Returns
    /// false once a budget is exceeded.
    pub fn observe_frame(&mut self, tick: u64, frame: &FrameCounters) -> bool {
        let peak = self.peak_frame.get_or_insert(*frame);
        peak.draws = peak.draws.max(frame.draws);
        peak.pipeline_switches = peak.pipeline_switches.max(frame.pipeline_switches);
        peak.bind_group_switches = peak.bind_group_switches.max(frame.bind_group_switches);
        peak.upload_bytes = peak.upload_bytes.max(frame.upload_bytes);
        let used = [
            u64::from(frame.draws),
            u64::from(frame.pipeline_switches),
            u64::from(frame.bind_group_switches),
            frame.upload_bytes,
        ];
        let limits = self.gpu_limits();
        self.check(
            tick,
            [0, 1, 2, 3].map(|i| (limits[i].0, limits[i].1, used[i])),
        )
    }

    fn check<const N: usize>(&mut self, tick: u64, checks: [(&str, Option<u64>, u64); N]) -> bool {
        for (name, limit, used) in checks {
            if let Some(limit) = limit.filter(|&limit| used > limit) {
                self.exceeded.get_or_insert_with(|| {
                    format!("[E-BUDGET] {name} exceeded at tick {tick}: {used} > {limit}")
                });
            }
        }
        self.exceeded.is_none()
    }

    pub fn limits(&self) -> (Option<u64>, Option<u64>) {
        (self.limits.max_state_bytes, self.limits.max_entities)
    }

    /// `[gpu]`'s limits, by their names in `budgets.toml`.
    pub fn gpu_limits(&self) -> [(&'static str, Option<u64>); 4] {
        [
            ("max_draws_per_frame", self.gpu.max_draws_per_frame),
            (
                "max_pipeline_switches_per_frame",
                self.gpu.max_pipeline_switches_per_frame,
            ),
            (
                "max_bind_group_switches_per_frame",
                self.gpu.max_bind_group_switches_per_frame,
            ),
            (
                "max_upload_bytes_per_frame",
                self.gpu.max_upload_bytes_per_frame,
            ),
        ]
    }
}

#[cfg(test)]
mod tests {
    use eng_gpu::FrameCounters;

    use super::BudgetTracker;

    /// Amendment 30's `[gpu]` table (rung 1b), as the owner would add it.
    const WITH_GPU: &str = "version = 1

[sim]
max_state_bytes = 4096
max_entities = 16

[gpu]
max_draws_per_frame = 12
max_pipeline_switches_per_frame = 12
max_bind_group_switches_per_frame = 8
max_upload_bytes_per_frame = 2048

[ci]
pr_tier_minutes = 15
native_job_minutes = 25
";

    fn frame(draws: u32, pipelines: u32, bind_groups: u32, bytes: u64) -> FrameCounters {
        FrameCounters {
            draws,
            pipeline_switches: pipelines,
            bind_group_switches: bind_groups,
            upload_bytes: bytes,
        }
    }

    #[test]
    fn the_repo_budgets_file_loads() {
        // Only that it loads: the values are the owner's to change.
        let text = include_str!("../../../budgets.toml");
        BudgetTracker::load(text, "budgets.toml").unwrap();
    }

    #[test]
    fn a_gpu_table_is_read() {
        let tracker = BudgetTracker::load(WITH_GPU, "budgets.toml").unwrap();
        assert_eq!(tracker.limits(), (Some(4096), Some(16)));
        assert_eq!(
            tracker.gpu_limits(),
            [
                ("max_draws_per_frame", Some(12)),
                ("max_pipeline_switches_per_frame", Some(12)),
                ("max_bind_group_switches_per_frame", Some(8)),
                ("max_upload_bytes_per_frame", Some(2048)),
            ]
        );
        let without = BudgetTracker::load("version = 1\n", "b.toml").unwrap();
        assert!(
            without
                .gpu_limits()
                .iter()
                .all(|(_, limit)| limit.is_none())
        );
    }

    #[test]
    fn an_unknown_gpu_key_is_refused() {
        let text = "version = 1\n[gpu]\nmax_triangles_per_frame = 3\n";
        let err = BudgetTracker::load(text, "b.toml").unwrap_err().to_string();
        assert!(
            err.contains("[E-LOAD]") && err.contains("max_triangles_per_frame"),
            "{err}"
        );
    }

    #[test]
    fn each_frame_counter_is_held_to_its_limit() {
        let at_limit = frame(12, 12, 8, 2048);
        let over = [
            (frame(13, 12, 8, 2048), "max_draws_per_frame", "13 > 12"),
            (
                frame(12, 13, 8, 2048),
                "max_pipeline_switches_per_frame",
                "13 > 12",
            ),
            (
                frame(12, 12, 9, 2048),
                "max_bind_group_switches_per_frame",
                "9 > 8",
            ),
            (
                frame(12, 12, 8, 2049),
                "max_upload_bytes_per_frame",
                "2049 > 2048",
            ),
        ];
        for (counters, name, numbers) in over {
            let mut tracker = BudgetTracker::load(WITH_GPU, "budgets.toml").unwrap();
            assert!(
                tracker.observe_frame(6, &at_limit),
                "at the limit is within"
            );
            assert_eq!(tracker.exceeded, None);
            assert!(!tracker.observe_frame(7, &counters), "{name}");
            assert_eq!(
                tracker.exceeded.as_deref(),
                Some(format!("[E-BUDGET] {name} exceeded at tick 7: {numbers}").as_str())
            );
            // The first excess is the one reported.
            assert!(!tracker.observe_frame(8, &frame(99, 99, 99, 99_999)));
            assert!(tracker.exceeded.as_deref().unwrap().contains(name));
        }
    }

    #[test]
    fn frame_peaks_are_kept_per_counter() {
        let mut tracker = BudgetTracker::unlimited();
        assert_eq!(tracker.peak_frame, None, "no frame yet");
        assert!(tracker.observe_frame(0, &frame(2, 5, 1, 64)));
        assert!(tracker.observe_frame(1, &frame(3, 2, 1, 32)));
        assert!(tracker.observe_frame(2, &frame(1, 1, 4, 96)));
        assert_eq!(tracker.peak_frame, Some(frame(3, 5, 4, 96)));
        assert_eq!(tracker.exceeded, None, "nothing is limited");
    }
}
