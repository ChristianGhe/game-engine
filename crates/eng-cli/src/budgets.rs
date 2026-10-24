//! Simulation budgets from `budgets.toml` (a protected oracle). Only
//! runner-independent quantities are budgeted here; wall-clock budgets need
//! real hardware.

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

/// Peak usage against the limits.
pub(crate) struct BudgetTracker {
    limits: SimBudgets,
    pub peak_state_bytes: u64,
    pub peak_entities: u64,
    pub exceeded: Option<String>,
}

impl BudgetTracker {
    pub fn unlimited() -> Self {
        Self::new(SimBudgets::default())
    }

    fn new(limits: SimBudgets) -> Self {
        Self {
            limits,
            peak_state_bytes: 0,
            peak_entities: 0,
            exceeded: None,
        }
    }

    pub fn load(text: &str, origin: &str) -> Result<Self, LoadError> {
        let file: BudgetFile = load_toml(text, origin)?;
        Ok(Self::new(file.sim))
    }

    /// Records one tick's usage. Returns false once a budget is exceeded.
    pub fn observe(&mut self, tick: u64, state_bytes: u64, entities: u64) -> bool {
        self.peak_state_bytes = self.peak_state_bytes.max(state_bytes);
        self.peak_entities = self.peak_entities.max(entities);
        let checks = [
            ("max_state_bytes", self.limits.max_state_bytes, state_bytes),
            ("max_entities", self.limits.max_entities, entities),
        ];
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
}
