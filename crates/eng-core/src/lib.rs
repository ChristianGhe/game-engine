//! Foundation types shared by every layer: ticks, [`PlayerCommand`] and
//! generational ids.
#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

use alloc::vec::Vec;
use core::fmt;

use eng_schema_core::Schema;

/// A simulation tick. Tick 0 is the initial state; stepping with the
/// commands for tick N produces the state at tick N.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Schema)]
#[schema(name = "eng.core.Tick", version = 1)]
pub struct Tick(pub u64);

impl Tick {
    pub const ZERO: Self = Self(0);

    /// # Panics
    /// On overflow, which would take 9.7 billion years at 60 Hz.
    pub fn next(self) -> Self {
        Self(self.0.checked_add(1).expect("tick counter overflow"))
    }
}

impl fmt::Display for Tick {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "tick {}", self.0)
    }
}

/// The one input type that humans, agents, the network and recordings all
/// produce (spec 6.4). Quantized before it reaches the simulation, live and
/// on replay.
///
/// View angles are absolute 16-bit binary angles, as in Quake 3's usercmd, so
/// repeating the last command is a safe policy for a missing one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Schema)]
#[schema(name = "eng.core.PlayerCommand", version = 1)]
pub struct PlayerCommand {
    pub tick: Tick,
    /// Game-defined button bitfield.
    pub buttons: u32,
    pub forward: i8,
    pub side: i8,
    pub view_yaw: u16,
    pub view_pitch: u16,
    pub select: u8,
}

impl PlayerCommand {
    /// No buttons, no movement, looking along +X.
    pub const fn idle(tick: Tick) -> Self {
        Self {
            tick,
            buttons: 0,
            forward: 0,
            side: 0,
            view_yaw: 0,
            view_pitch: 0,
            select: 0,
        }
    }

    /// The same command restamped for `tick`: the missing-command policy.
    pub const fn repeated_at(self, tick: Tick) -> Self {
        Self { tick, ..self }
    }
}

/// Which player's commands drive an entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Schema)]
#[schema(name = "eng.core.PlayerSlot", version = 1)]
pub struct PlayerSlot(pub u8);

/// A generational id: an index into some table plus the generation that
/// detects stale references after the slot is reused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Schema)]
#[schema(name = "eng.core.GenId", version = 1)]
pub struct GenId {
    pub index: u32,
    pub generation: u32,
}

impl fmt::Display for GenId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}v{}", self.index, self.generation)
    }
}

/// Hands out [`GenId`]s from a deterministic last-in, first-out free list.
///
/// A slot whose generation reaches `u32::MAX` is retired rather than reused,
/// so a stale id can never alias a live one.
#[derive(Debug, Clone, PartialEq, Eq, Default, Schema)]
#[schema(name = "eng.core.GenAllocator", version = 1)]
pub struct GenAllocator {
    generations: Vec<u32>,
    alive: Vec<bool>,
    free: Vec<u32>,
}

impl GenAllocator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn alloc(&mut self) -> GenId {
        if let Some(index) = self.free.pop() {
            self.alive[index as usize] = true;
            return GenId {
                index,
                generation: self.generations[index as usize],
            };
        }
        let index = u32::try_from(self.generations.len()).expect("more than 2^32 ids");
        self.generations.push(0);
        self.alive.push(true);
        GenId {
            index,
            generation: 0,
        }
    }

    /// Frees `id`. Returns false if it was not alive.
    pub fn free(&mut self, id: GenId) -> bool {
        if !self.is_alive(id) {
            return false;
        }
        let i = id.index as usize;
        self.alive[i] = false;
        if self.generations[i] < u32::MAX {
            self.generations[i] += 1;
            self.free.push(id.index);
        }
        true
    }

    pub fn is_alive(&self, id: GenId) -> bool {
        let i = id.index as usize;
        i < self.alive.len() && self.alive[i] && self.generations[i] == id.generation
    }

    /// Live ids in index order.
    pub fn iter_alive(&self) -> impl Iterator<Item = GenId> + '_ {
        self.alive
            .iter()
            .enumerate()
            .filter(|(_, alive)| **alive)
            .map(|(i, _)| GenId {
                index: i as u32,
                generation: self.generations[i],
            })
    }

    pub fn alive_count(&self) -> usize {
        self.alive.iter().filter(|a| **a).count()
    }

    /// Checks the internal invariants; used after decoding untrusted bytes.
    pub fn is_consistent(&self) -> bool {
        if self.alive.len() != self.generations.len() {
            return false;
        }
        let mut seen = alloc::collections::BTreeSet::new();
        self.free.iter().all(|&i| {
            let i = i as usize;
            i < self.alive.len() && !self.alive[i] && seen.insert(i)
        })
    }
}
