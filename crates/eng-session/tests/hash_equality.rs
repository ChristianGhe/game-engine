//! The session gives the same state hashes as stepping the simulation
//! directly (spec 6.8; rung 1b criterion 11, `wire-session`). PROTECTED
//! ORACLE once the owner applies `docs/rungs/1b/patches/t6-oracles.patch`:
//! "one code path" is safe only while a session over Memory, with every
//! message encoded to bytes, changes nothing in the simulation. The test sim
//! is defined here and not shared, so nothing outside this file can weaken
//! the check.
//!
//! - Two clients drive the two player slots. At every tick from 0, the
//!   server's state and each client's restored state hash the same as a sim
//!   stepped directly with the same commands.
//! - Commands sent early or never sent: the server applies each command at
//!   its tick and repeats a slot's last command when one is missing (idle
//!   before the first). Stepping directly with those commands gives the
//!   same hashes, and they are exactly the commands the server says it
//!   applied, which is what recordings replay.

use eng_core::{PlayerCommand, PlayerSlot, Tick};
use eng_ecs::{Schema, World};
use eng_net_transport::memory::{MemoryEndpoint, MemoryNetwork};
use eng_session::{Client, Server};
use eng_sim::{GameplayRng, Sim, SimBuilder, SimConfig, SimInfo, Stage, System, TickInputs};
use proptest::prelude::*;

const PLAYERS: u8 = 2;
const SEED: u64 = 0x5e55_1017;

/// One body per player slot, moved by its commands and the gameplay random
/// stream. Every command field changes the state.
#[derive(Debug, Clone, Copy, PartialEq, Schema)]
#[schema(name = "test.Body", version = 1)]
struct Body {
    x: f64,
    y: f64,
    vx: f32,
    vy: f32,
    view: u32,
}

/// Sent when a command holds buttons, so events travel in snapshots too.
#[derive(Debug, Clone, Copy, PartialEq, Schema)]
#[schema(name = "test.Pressed", version = 1)]
struct Pressed {
    slot: u8,
    buttons: u32,
}

fn build() -> Sim {
    let mut builder = SimBuilder::new(SimConfig {
        tick_rate: 60,
        players: PLAYERS,
        seed: SEED,
    });
    let world = builder.world_mut();
    world.register_component::<PlayerSlot>();
    world.register_component::<Body>();
    world.register_event::<Pressed>();
    for slot in 0..PLAYERS {
        let rng = &mut world.resource_mut::<GameplayRng>().0;
        let body = Body {
            x: 8.0 * rng.next_f64(),
            y: 6.0 * rng.next_f64(),
            vx: 0.0,
            vy: 0.0,
            view: rng.next_u32(),
        };
        let entity = world.spawn();
        world.insert(entity, PlayerSlot(slot));
        world.insert(entity, body);
    }
    builder.add_system(System::new("test.drive", Stage::Update, drive));
    builder.build().expect("the test schedule resolves")
}

fn drive(world: &mut World) {
    let dt = world.resource::<SimInfo>().dt();
    let inputs = world.resource::<TickInputs>().clone();
    let gust = world.resource_mut::<GameplayRng>().0.next_f32() - 0.5;
    let mut pressed = Vec::new();
    let (slots, bodies) = world.storage_pair_mut::<PlayerSlot, Body>();
    for (entity, body) in bodies.iter_mut() {
        let slot = *slots.get(entity).expect("every body has a slot");
        let cmd = inputs.for_player(slot).expect("one command per slot");
        body.vx = 0.9 * body.vx + dt * f32::from(cmd.forward) + gust;
        body.vy = 0.9 * body.vy + dt * f32::from(cmd.side) - gust;
        body.x += f64::from(body.vx) * f64::from(dt);
        body.y += f64::from(body.vy) * f64::from(dt);
        body.view =
            (u32::from(cmd.view_yaw) << 16 | u32::from(cmd.view_pitch)) ^ u32::from(cmd.select);
        if cmd.buttons != 0 {
            pressed.push(Pressed {
                slot: slot.0,
                buttons: cmd.buttons,
            });
        }
    }
    for event in pressed {
        world.send_event(event);
    }
}

/// A command for `slot` at `tick` in which every field varies.
fn command(slot: u8, tick: u64) -> PlayerCommand {
    let n = tick * 31 + u64::from(slot) * 17;
    PlayerCommand {
        tick: Tick(tick),
        buttons: if n.is_multiple_of(5) { 1 << (n % 3) } else { 0 },
        forward: (n % 256) as u8 as i8,
        side: (n / 3 % 256) as u8 as i8,
        view_yaw: (n * 1021) as u16,
        view_pitch: (n * 13) as u16,
        select: (n % 7) as u8,
    }
}

struct Session {
    server: Server<MemoryEndpoint>,
    clients: Vec<Client<MemoryEndpoint>>,
}

impl Session {
    /// A server and one client per slot over Memory, past the handshake.
    fn connect() -> Self {
        let net = MemoryNetwork::new();
        let mut server = Server::new(build(), net.endpoint());
        let mut clients: Vec<Client<MemoryEndpoint>> = (0..PLAYERS)
            .map(|_| Client::connect(build(), net.endpoint(), server.local_id()).unwrap())
            .collect();
        server.poll().unwrap();
        for client in &mut clients {
            client.poll().unwrap();
        }
        Self { server, clients }
    }

    fn slot(client: &Client<MemoryEndpoint>) -> usize {
        usize::from(client.welcome().expect("welcomed").slot.0)
    }

    /// The server's state and every client's equal `direct`'s, by hash.
    fn check(&self, direct: &Sim) {
        let tick = direct.tick();
        let want = direct.state_hash().unwrap();
        assert_eq!(self.server.sim().tick(), tick);
        assert_eq!(
            self.server.sim().state_hash().unwrap(),
            want,
            "server at {tick}"
        );
        for client in &self.clients {
            assert_eq!(client.latest(), Some(tick), "client's newest snapshot");
            assert_eq!(
                client.sim().state_hash().unwrap(),
                want,
                "client in slot {} at {tick}",
                Self::slot(client)
            );
        }
    }
}

#[test]
fn session_hashes_equal_stepping_directly() {
    let mut session = Session::connect();
    let mut slots: Vec<usize> = session.clients.iter().map(Session::slot).collect();
    slots.sort_unstable();
    assert_eq!(slots, [0, 1], "one client per slot");
    let mut direct = build();
    session.check(&direct);
    for tick in 1..=240 {
        let commands: Vec<PlayerCommand> = (0..PLAYERS).map(|slot| command(slot, tick)).collect();
        for client in &mut session.clients {
            let slot = Session::slot(client);
            client.send(commands[slot]).unwrap();
        }
        session.server.poll().unwrap();
        let applied = session.server.step().unwrap().to_vec();
        for client in &mut session.clients {
            client.poll().unwrap();
        }
        direct.step(&commands).unwrap();
        assert_eq!(applied, commands, "applied at tick {tick}");
        session.check(&direct);
    }
}

/// One client's script: its command for each tick from 1 (`None`: never
/// sent), and how many ticks early it sends them.
#[derive(Debug, Clone)]
struct Script {
    commands: Vec<Option<PlayerCommand>>,
    lead: u64,
}

fn script(ticks: usize) -> impl Strategy<Value = Script> {
    let field = (
        any::<u32>(),
        any::<i8>(),
        any::<i8>(),
        any::<u16>(),
        any::<u16>(),
        any::<u8>(),
    );
    let entry = prop::option::weighted(0.75, field);
    (prop::collection::vec(entry, ticks), 0..=3u64).prop_map(|(fields, lead)| Script {
        commands: fields
            .into_iter()
            .enumerate()
            .map(|(i, f)| {
                f.map(
                    |(buttons, forward, side, view_yaw, view_pitch, select)| PlayerCommand {
                        tick: Tick(i as u64 + 1),
                        buttons,
                        forward,
                        side,
                        view_yaw,
                        view_pitch,
                        select,
                    },
                )
            })
            .collect(),
        lead,
    })
}

fn scripts() -> impl Strategy<Value = Vec<Script>> {
    (1..=90usize).prop_flat_map(|ticks| prop::collection::vec(script(ticks), usize::from(PLAYERS)))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    /// Whatever each client sends, and however early, stepping directly
    /// with the commands the server applied gives the session's hashes, and
    /// those commands follow the missing-command rule.
    #[test]
    fn missing_and_early_commands_still_match_stepping_directly(scripts in scripts()) {
        let mut session = Session::connect();
        let ticks = scripts[0].commands.len() as u64;
        // The rule, written out: a slot's command, else its last one repeated.
        let mut last: Vec<PlayerCommand> =
            (0..PLAYERS).map(|_| PlayerCommand::idle(Tick::ZERO)).collect();
        let mut direct = build();
        session.check(&direct);
        for tick in 1..=ticks {
            for client in &mut session.clients {
                let script = &scripts[Session::slot(client)];
                // Sent `lead` ticks before it is due, each command once.
                let due = tick + script.lead;
                let first = if tick == 1 { 1 } else { due };
                for t in first..=due.min(ticks) {
                    if let Some(cmd) = script.commands[t as usize - 1] {
                        client.send(cmd).unwrap();
                    }
                }
            }
            session.server.poll().unwrap();
            let applied = session.server.step().unwrap().to_vec();
            for client in &mut session.clients {
                client.poll().unwrap();
            }
            for (slot, last) in last.iter_mut().enumerate() {
                *last = scripts[slot].commands[tick as usize - 1]
                    .unwrap_or(last.repeated_at(Tick(tick)));
            }
            prop_assert_eq!(&applied, &last, "applied at tick {}", tick);
            direct.step(&last).unwrap();
            session.check(&direct);
        }
    }
}
