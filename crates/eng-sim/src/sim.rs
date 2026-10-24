use alloc::vec::Vec;
use core::fmt;

use eng_core::{PlayerCommand, PlayerSlot, Tick};
use eng_ecs::World;
use eng_math::{Fnv1a64, Pcg32};
use eng_schema_core::{DecodeError, Encoder, NonFinite, Schema, Sink, find_non_finite};

use crate::schedule::{ScheduleError, Stage, System, resolve};

/// PCG32 stream for gameplay randomness (Doom's `P_Random`).
pub const GAMEPLAY_STREAM: u64 = 0x6761_6d65_706c_6179; // "gameplay"
/// PCG32 stream for cosmetic randomness (Doom's `M_Random`). Cosmetic
/// randomness lives outside the simulation and never feeds back into it.
pub const COSMETIC_STREAM: u64 = 0x636f_736d_6574_6963; // "cosmetic"

/// The cosmetic generator for a session seed.
pub fn cosmetic_rng(seed: u64) -> Pcg32 {
    Pcg32::new(seed, COSMETIC_STREAM)
}

/// The current tick, as a resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Schema)]
#[schema(name = "eng.sim.SimTick", version = 1)]
pub struct SimTick(pub Tick);

/// Session constants, as a resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Schema)]
#[schema(name = "eng.sim.SimInfo", version = 1)]
pub struct SimInfo {
    pub tick_rate: u32,
    pub players: u8,
    pub seed: u64,
}

impl SimInfo {
    /// The fixed tick length in seconds.
    pub fn dt(&self) -> f32 {
        1.0 / self.tick_rate as f32
    }
}

/// This tick's commands, one per player slot.
#[derive(Debug, Clone, PartialEq, Eq, Default, Schema)]
#[schema(name = "eng.sim.TickInputs", version = 1)]
pub struct TickInputs {
    pub commands: Vec<PlayerCommand>,
}

impl TickInputs {
    pub fn for_player(&self, slot: PlayerSlot) -> Option<&PlayerCommand> {
        self.commands.get(usize::from(slot.0))
    }
}

/// The gameplay random stream, as a resource.
#[derive(Debug, Clone, PartialEq, Eq, Schema)]
#[schema(name = "eng.sim.GameplayRng", version = 1)]
pub struct GameplayRng(pub Pcg32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SimConfig {
    pub tick_rate: u32,
    pub players: u8,
    pub seed: u64,
}

/// Collects registrations and systems, then resolves them into a [`Sim`].
pub struct SimBuilder {
    world: World,
    systems: Vec<System>,
    config: SimConfig,
}

impl SimBuilder {
    /// # Panics
    /// If the tick rate or the player count is zero.
    pub fn new(config: SimConfig) -> Self {
        assert!(config.tick_rate > 0, "tick rate must be positive");
        assert!(config.players > 0, "a session needs at least one player");
        let mut world = World::new();
        world.insert_resource(SimTick(Tick::ZERO));
        world.insert_resource(SimInfo {
            tick_rate: config.tick_rate,
            players: config.players,
            seed: config.seed,
        });
        world.insert_resource(TickInputs::default());
        world.insert_resource(GameplayRng(Pcg32::new(config.seed, GAMEPLAY_STREAM)));
        Self {
            world,
            systems: Vec::new(),
            config,
        }
    }

    /// The world, for registrations and initial entities.
    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }

    pub fn add_system(&mut self, system: System) -> &mut Self {
        self.systems.push(system);
        self
    }

    pub fn build(mut self) -> Result<Sim, ScheduleError> {
        let systems = resolve(self.systems)?;
        self.world.apply_deferred();
        let protocol = protocol_version(&self.world, &systems, &self.config);
        Ok(Sim {
            world: self.world,
            systems,
            config: self.config,
            protocol,
        })
    }
}

/// A running simulation.
pub struct Sim {
    world: World,
    systems: Vec<System>,
    config: SimConfig,
    protocol: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepError {
    PlayerCount {
        expected: u8,
        got: usize,
    },
    WrongTick {
        player: usize,
        expected: Tick,
        got: Tick,
    },
}

impl fmt::Display for StepError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PlayerCount { expected, got } => {
                write!(f, "expected {expected} player commands, got {got}")
            }
            Self::WrongTick {
                player,
                expected,
                got,
            } => write!(
                f,
                "player {player}'s command is for {got}, expected {expected}"
            ),
        }
    }
}

impl core::error::Error for StepError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RestoreError {
    Protocol {
        expected: u64,
        found: u64,
    },
    Decode(DecodeError),
    /// The snapshot is from a session with other constants.
    Session {
        expected: SimInfo,
        found: SimInfo,
    },
    NonFinite(NonFinite),
}

impl fmt::Display for RestoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Protocol { expected, found } => write!(
                f,
                "snapshot is for simulation protocol {found:#018x}, this build is {expected:#018x}"
            ),
            Self::Decode(err) => write!(f, "snapshot does not decode: {err}"),
            Self::Session { expected, found } => write!(
                f,
                "snapshot is for session {found:?}, this sim is {expected:?}"
            ),
            Self::NonFinite(bad) => write!(f, "snapshot holds {bad}"),
        }
    }
}

impl core::error::Error for RestoreError {}

impl Sim {
    pub fn config(&self) -> SimConfig {
        self.config
    }

    pub fn tick(&self) -> Tick {
        self.world.resource::<SimTick>().0
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    /// Hash of the registered schemas, the resolved system order and the
    /// tick rate. Recordings and hash streams are valid only for one value.
    pub fn protocol_version(&self) -> u64 {
        self.protocol
    }

    /// The resolved order, stage by stage.
    pub fn system_order(&self) -> impl Iterator<Item = (Stage, &'static str)> + '_ {
        self.systems.iter().map(|s| (s.stage(), s.name()))
    }

    /// Advances one tick. `commands` holds one command per player slot, each
    /// stamped with the tick being produced (`self.tick() + 1`).
    pub fn step(&mut self, commands: &[PlayerCommand]) -> Result<(), StepError> {
        let tick = self.tick().next();
        if commands.len() != usize::from(self.config.players) {
            return Err(StepError::PlayerCount {
                expected: self.config.players,
                got: commands.len(),
            });
        }
        if let Some((player, cmd)) = commands.iter().enumerate().find(|(_, c)| c.tick != tick) {
            return Err(StepError::WrongTick {
                player,
                expected: tick,
                got: cmd.tick,
            });
        }
        self.world.resource_mut::<SimTick>().0 = tick;
        let inputs = &mut self.world.resource_mut::<TickInputs>().commands;
        inputs.clear();
        inputs.extend_from_slice(commands);
        self.world.clear_events();
        for stage in Stage::ALL {
            for system in self.systems.iter().filter(|s| s.stage() == stage) {
                (system.run)(&mut self.world);
                #[cfg(debug_assertions)]
                if let Some(bad) = find_non_finite(&|v| self.world.visit_dump(v)) {
                    panic!("[E-NAN] after system {} at {tick}: {bad}", system.name());
                }
            }
            self.world.apply_deferred();
        }
        Ok(())
    }

    /// FNV-1a-64 over the canonical encoding of the whole world. Fails with
    /// the entity and field path if any float is NaN or infinite.
    pub fn state_hash(&self) -> Result<u64, NonFinite> {
        if let Some(bad) = find_non_finite(&|v| self.world.visit_dump(v)) {
            return Err(bad);
        }
        let mut hash = Fnv1a64::new();
        self.world.visit_state(&mut Encoder::new(&mut hash));
        Ok(hash.finish())
    }

    /// The world's canonical encoding.
    pub fn state_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        self.world.visit_state(&mut Encoder::new(&mut out));
        out
    }

    /// The length of the canonical encoding, without allocating it.
    pub fn state_len(&self) -> usize {
        let mut counter = CountingSink(0);
        self.world.visit_state(&mut Encoder::new(&mut counter));
        counter.0
    }

    /// The protocol version (u64, little-endian) followed by the state.
    pub fn snapshot(&self) -> Vec<u8> {
        let mut out = Vec::from(self.protocol.to_le_bytes());
        self.world.visit_state(&mut Encoder::new(&mut out));
        out
    }

    /// Replaces the state with a snapshot from a build with the same protocol
    /// and a session with the same constants. All-or-nothing: on error the
    /// state is unchanged.
    pub fn restore(&mut self, snapshot: &[u8]) -> Result<(), RestoreError> {
        let mut reader = eng_schema_core::Reader::new(snapshot);
        let found = reader.read_u64().map_err(RestoreError::Decode)?;
        if found != self.protocol {
            return Err(RestoreError::Protocol {
                expected: self.protocol,
                found,
            });
        }
        let backup = self.state_bytes();
        self.world
            .decode_state(&snapshot[8..])
            .map_err(RestoreError::Decode)?;
        let info = *self.world.resource::<SimInfo>();
        let expected = SimInfo {
            tick_rate: self.config.tick_rate,
            players: self.config.players,
            seed: self.config.seed,
        };
        let problem = if info != expected {
            Some(RestoreError::Session {
                expected,
                found: info,
            })
        } else {
            find_non_finite(&|v| self.world.visit_dump(v)).map(RestoreError::NonFinite)
        };
        if let Some(problem) = problem {
            self.world
                .decode_state(&backup)
                .expect("the state this sim encoded decodes again");
            return Err(problem);
        }
        Ok(())
    }
}

struct CountingSink(usize);

impl Sink for CountingSink {
    fn put(&mut self, bytes: &[u8]) {
        self.0 += bytes.len();
    }
}

fn protocol_version(world: &World, systems: &[System], config: &SimConfig) -> u64 {
    fn put_str(h: &mut Fnv1a64, s: &str) {
        h.write(&(s.len() as u64).to_le_bytes());
        h.write(s.as_bytes());
    }
    let mut h = Fnv1a64::new();
    put_str(&mut h, "eng.sim.protocol.v1");
    h.write(&config.tick_rate.to_le_bytes());
    let kinds: [(&str, Vec<&str>); 3] = [
        ("components", world.component_names().collect()),
        ("resources", world.resource_names().collect()),
        ("events", world.event_names().collect()),
    ];
    for (label, names) in kinds {
        put_str(&mut h, label);
        h.write(&(names.len() as u64).to_le_bytes());
        for name in names {
            put_str(&mut h, name);
        }
    }
    world.registry().encode(&mut h);
    h.write(&(systems.len() as u64).to_le_bytes());
    for system in systems {
        h.write(&[system.stage().index()]);
        put_str(&mut h, system.name());
    }
    h.finish()
}
