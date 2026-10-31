//! Test sims and raw endpoints for the session's behaviour tests. Each test
//! file uses only some of them.
#![allow(dead_code)]

use eng_core::{PlayerSlot, Tick};
use eng_ecs::{Schema, World};
use eng_net_transport::memory::MemoryEndpoint;
use eng_net_transport::{PeerId, Transport};
use eng_net_wire::Message;
use eng_sim::{Sim, SimBuilder, SimConfig, SimTick, Stage, System, TickInputs};

pub const SEED: u64 = 7;

/// How far a slot's player has walked: the sum of its commands' `forward`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Schema)]
#[schema(name = "test.Walked", version = 1)]
pub struct Walked(pub i64);

/// Filler bytes that grow by `growth` every tick, to size snapshots.
#[derive(Debug, Clone, PartialEq, Eq, Schema)]
#[schema(name = "test.Blob", version = 1)]
pub struct Blob {
    pub bytes: Vec<u8>,
    pub growth: u32,
}

fn walk(world: &mut World) {
    let inputs = world.resource::<TickInputs>().clone();
    let (slots, walked) = world.storage_pair_mut::<PlayerSlot, Walked>();
    for (entity, walked) in walked.iter_mut() {
        let slot = *slots.get(entity).expect("every walker has a slot");
        walked.0 += i64::from(inputs.for_player(slot).expect("one per slot").forward);
    }
}

fn grow(world: &mut World) {
    let blob = world.resource_mut::<Blob>();
    let n = blob.growth as usize;
    blob.bytes.resize(blob.bytes.len() + n, 0xb1);
}

/// A sim with one walker per player, starting at `start`. The seed is part
/// of the state, not the protocol.
pub fn sim_with(players: u8, seed: u64, start: Tick) -> Sim {
    let mut builder = SimBuilder::new(SimConfig {
        tick_rate: 60,
        players,
        seed,
    });
    let world = builder.world_mut();
    world.register_component::<PlayerSlot>();
    world.register_component::<Walked>();
    world.insert_resource(SimTick(start));
    for slot in 0..players {
        let entity = world.spawn();
        world.insert(entity, PlayerSlot(slot));
        world.insert(entity, Walked(0));
    }
    builder.add_system(System::new("test.walk", Stage::Update, walk));
    builder.build().expect("the test schedule resolves")
}

pub fn sim_at(players: u8, start: Tick) -> Sim {
    sim_with(players, SEED, start)
}

pub fn sim(players: u8) -> Sim {
    sim_at(players, Tick::ZERO)
}

/// A one-player sim whose state holds `len` filler bytes, growing by
/// `growth` a tick. Its protocol differs from [`sim`]'s.
pub fn blob_sim(len: usize, growth: u32) -> Sim {
    let mut builder = SimBuilder::new(SimConfig {
        tick_rate: 60,
        players: 1,
        seed: SEED,
    });
    builder.world_mut().insert_resource(Blob {
        bytes: vec![0xb1; len],
        growth,
    });
    builder.add_system(System::new("test.grow", Stage::Update, grow));
    builder.build().expect("the test schedule resolves")
}

pub fn walked(sim: &Sim, slot: u8) -> i64 {
    let world = sim.world();
    let slots = world.storage::<PlayerSlot>();
    world
        .storage::<Walked>()
        .iter()
        .find(|(entity, _)| slots.get(*entity) == Some(&PlayerSlot(slot)))
        .map(|(_, walked)| walked.0)
        .expect("a walker for the slot")
}

pub fn send(endpoint: &mut MemoryEndpoint, to: PeerId, message: Message) {
    endpoint.send(to, &message.encode().unwrap()).unwrap();
}

/// Every waiting datagram, decoded against `baseline`.
pub fn receive(endpoint: &mut MemoryEndpoint, baseline: Tick) -> Vec<(PeerId, Message)> {
    std::iter::from_fn(|| endpoint.poll_recv())
        .map(|(from, bytes)| (from, Message::decode(&bytes, baseline).unwrap()))
        .collect()
}
