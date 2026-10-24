use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;
use core::fmt;

use eng_ecs::World;

/// The fixed stages of every tick, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Stage {
    Input,
    PreUpdate,
    Update,
    PostUpdate,
    Commit,
}

impl Stage {
    pub const ALL: [Stage; 5] = [
        Stage::Input,
        Stage::PreUpdate,
        Stage::Update,
        Stage::PostUpdate,
        Stage::Commit,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Stage::Input => "Input",
            Stage::PreUpdate => "PreUpdate",
            Stage::Update => "Update",
            Stage::PostUpdate => "PostUpdate",
            Stage::Commit => "Commit",
        }
    }

    pub(crate) const fn index(self) -> u8 {
        self as u8
    }
}

pub type SystemFn = fn(&mut World);

/// A system: a named plain function placed in a stage.
///
/// Names are stable dotted strings like `"testbed.move_dots"`; they break
/// ordering ties and are hashed into the protocol version.
#[derive(Clone)]
pub struct System {
    name: &'static str,
    stage: Stage,
    pub(crate) run: SystemFn,
    before: Vec<&'static str>,
    after: Vec<&'static str>,
}

impl System {
    pub fn new(name: &'static str, stage: Stage, run: SystemFn) -> Self {
        Self {
            name,
            stage,
            run,
            before: Vec::new(),
            after: Vec::new(),
        }
    }

    /// Runs before `other`, which must be in the same stage.
    pub fn before(mut self, other: &'static str) -> Self {
        self.before.push(other);
        self
    }

    /// Runs after `other`, which must be in the same stage.
    pub fn after(mut self, other: &'static str) -> Self {
        self.after.push(other);
        self
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn stage(&self) -> Stage {
        self.stage
    }
}

impl fmt::Debug for System {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("System")
            .field("name", &self.name)
            .field("stage", &self.stage)
            .field("before", &self.before)
            .field("after", &self.after)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScheduleError {
    DuplicateName(&'static str),
    UnknownSystem {
        system: &'static str,
        refers_to: &'static str,
    },
    CrossStage {
        system: &'static str,
        other: &'static str,
    },
    Cycle {
        stage: Stage,
        systems: Vec<&'static str>,
    },
}

impl fmt::Display for ScheduleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateName(name) => write!(f, "two systems are named {name:?}"),
            Self::UnknownSystem { system, refers_to } => {
                write!(
                    f,
                    "{system:?} is ordered against unknown system {refers_to:?}"
                )
            }
            Self::CrossStage { system, other } => write!(
                f,
                "{system:?} is ordered against {other:?}, which is in another stage"
            ),
            Self::Cycle { stage, systems } => {
                write!(f, "ordering cycle in stage {}: {systems:?}", stage.name())
            }
        }
    }
}

impl core::error::Error for ScheduleError {}

/// Orders systems stage by stage. Within a stage: Kahn's algorithm, always
/// taking the ready system with the smallest name.
pub(crate) fn resolve(systems: Vec<System>) -> Result<Vec<System>, ScheduleError> {
    let mut by_name: BTreeMap<&'static str, usize> = BTreeMap::new();
    for (i, s) in systems.iter().enumerate() {
        if by_name.insert(s.name, i).is_some() {
            return Err(ScheduleError::DuplicateName(s.name));
        }
    }
    // edges[i] = systems that must run after i.
    let mut edges: Vec<BTreeSet<usize>> = alloc::vec![BTreeSet::new(); systems.len()];
    for (i, s) in systems.iter().enumerate() {
        let lookup = |other: &'static str| -> Result<usize, ScheduleError> {
            let j = *by_name.get(other).ok_or(ScheduleError::UnknownSystem {
                system: s.name,
                refers_to: other,
            })?;
            if systems[j].stage != s.stage {
                return Err(ScheduleError::CrossStage {
                    system: s.name,
                    other,
                });
            }
            Ok(j)
        };
        for &other in &s.before {
            let j = lookup(other)?;
            edges[i].insert(j);
        }
        for &other in &s.after {
            let j = lookup(other)?;
            edges[j].insert(i);
        }
    }
    let mut indegree = alloc::vec![0usize; systems.len()];
    for targets in &edges {
        for &j in targets {
            indegree[j] += 1;
        }
    }

    let mut order = Vec::with_capacity(systems.len());
    for stage in Stage::ALL {
        let members: Vec<usize> = (0..systems.len())
            .filter(|&i| systems[i].stage == stage)
            .collect();
        let mut ready: BTreeSet<(&'static str, usize)> = members
            .iter()
            .filter(|&&i| indegree[i] == 0)
            .map(|&i| (systems[i].name, i))
            .collect();
        let mut placed = 0;
        while let Some((_, i)) = ready.pop_first() {
            order.push(i);
            placed += 1;
            for &j in &edges[i] {
                indegree[j] -= 1;
                if indegree[j] == 0 {
                    ready.insert((systems[j].name, j));
                }
            }
        }
        if placed != members.len() {
            let mut stuck: Vec<&'static str> = members
                .iter()
                .filter(|&&i| indegree[i] > 0)
                .map(|&i| systems[i].name)
                .collect();
            stuck.sort_unstable();
            return Err(ScheduleError::Cycle {
                stage,
                systems: stuck,
            });
        }
    }

    let mut slots: Vec<Option<System>> = systems.into_iter().map(Some).collect();
    Ok(order
        .into_iter()
        .map(|i| slots[i].take().expect("each system placed once"))
        .collect())
}
