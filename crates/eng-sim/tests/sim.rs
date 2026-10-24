use eng_core::{PlayerCommand, PlayerSlot, Tick};
use eng_ecs::{Schema, World};
use eng_sim::*;

#[derive(Debug, Clone, Copy, PartialEq, Schema)]
#[schema(name = "test.Value", version = 1)]
struct Value(f32);

#[derive(Debug, Clone, Copy, PartialEq, Schema)]
#[schema(name = "test.Extra", version = 1)]
struct Extra(u8);

#[derive(Debug, Clone, Copy, PartialEq, Schema)]
#[schema(name = "test.Log", version = 1)]
struct Log(u64);

#[derive(Debug, Clone, Copy, PartialEq, Schema)]
#[schema(name = "test.Ticked", version = 1)]
struct Ticked(u64);

const CONFIG: SimConfig = SimConfig {
    tick_rate: 60,
    players: 1,
    seed: 7,
};

fn noop(_: &mut World) {}

fn append(digit: u64) -> SystemFn {
    match digit {
        1 => |w| w.resource_mut::<Log>().0 = w.resource::<Log>().0 * 10 + 1,
        2 => |w| w.resource_mut::<Log>().0 = w.resource::<Log>().0 * 10 + 2,
        _ => |w| w.resource_mut::<Log>().0 = w.resource::<Log>().0 * 10 + 3,
    }
}

fn integrate(w: &mut World) {
    let cmd = *w
        .resource::<TickInputs>()
        .for_player(PlayerSlot(0))
        .unwrap();
    let noise = w.resource_mut::<GameplayRng>().0.next_f32();
    let dt = w.resource::<SimInfo>().dt();
    w.resource_mut::<Value>().0 += f32::from(cmd.forward) * dt + noise;
    let tick = w.resource::<SimTick>().0;
    w.send_event(Ticked(tick.0));
}

fn builder(config: SimConfig) -> SimBuilder {
    let mut b = SimBuilder::new(config);
    b.world_mut().insert_resource(Value(0.0));
    b.world_mut().insert_resource(Log(0));
    b.world_mut().register_event::<Ticked>();
    b
}

fn sim(config: SimConfig) -> Sim {
    let mut b = builder(config);
    b.add_system(System::new("test.integrate", Stage::Update, integrate));
    b.build().unwrap()
}

fn cmd(tick: u64, forward: i8) -> PlayerCommand {
    PlayerCommand {
        forward,
        ..PlayerCommand::idle(Tick(tick))
    }
}

fn hashes(sim: &mut Sim, ticks: u64) -> Vec<u64> {
    let mut out = vec![sim.state_hash().unwrap()];
    for _ in 0..ticks {
        let t = sim.tick().0 + 1;
        sim.step(&[cmd(t, (t % 7) as i8)]).unwrap();
        out.push(sim.state_hash().unwrap());
    }
    out
}

#[test]
fn order_ignores_registration_order_and_breaks_ties_by_name() {
    let systems = || {
        [
            System::new("c", Stage::Update, append(3)),
            System::new("b", Stage::Update, append(2)),
            System::new("a", Stage::Update, append(1)).after("c"),
        ]
    };
    let mut forward = builder(CONFIG);
    for s in systems() {
        forward.add_system(s);
    }
    let mut reverse = builder(CONFIG);
    for s in systems().into_iter().rev() {
        reverse.add_system(s);
    }
    let (mut forward, mut reverse) = (forward.build().unwrap(), reverse.build().unwrap());
    let order: Vec<_> = forward.system_order().map(|(_, n)| n).collect();
    assert_eq!(order, ["b", "c", "a"]);
    assert_eq!(forward.protocol_version(), reverse.protocol_version());
    forward.step(&[cmd(1, 0)]).unwrap();
    reverse.step(&[cmd(1, 0)]).unwrap();
    assert_eq!(forward.world().resource::<Log>().0, 231);
    assert_eq!(reverse.world().resource::<Log>().0, 231);
}

#[test]
fn stages_run_in_fixed_order() {
    let mut b = builder(CONFIG);
    b.add_system(System::new("z.commit", Stage::Commit, append(3)));
    b.add_system(System::new("y.input", Stage::Input, append(1)));
    b.add_system(System::new("x.update", Stage::Update, append(2)));
    let mut s = b.build().unwrap();
    s.step(&[cmd(1, 0)]).unwrap();
    assert_eq!(s.world().resource::<Log>().0, 123);
}

#[test]
fn schedule_errors() {
    let build = |systems: Vec<System>| {
        let mut b = builder(CONFIG);
        for s in systems {
            b.add_system(s);
        }
        b.build().err()
    };
    assert_eq!(
        build(vec![
            System::new("a", Stage::Update, noop),
            System::new("a", Stage::Update, noop)
        ]),
        Some(ScheduleError::DuplicateName("a"))
    );
    assert!(matches!(
        build(vec![System::new("a", Stage::Update, noop).after("ghost")]),
        Some(ScheduleError::UnknownSystem { .. })
    ));
    assert!(matches!(
        build(vec![
            System::new("a", Stage::Update, noop).after("b"),
            System::new("b", Stage::Input, noop)
        ]),
        Some(ScheduleError::CrossStage { .. })
    ));
    assert_eq!(
        build(vec![
            System::new("a", Stage::Update, noop).after("b"),
            System::new("b", Stage::Update, noop).after("a"),
            System::new("c", Stage::Update, noop),
        ]),
        Some(ScheduleError::Cycle {
            stage: Stage::Update,
            systems: vec!["a", "b"]
        })
    );
}

#[test]
fn step_checks_commands() {
    let mut s = sim(CONFIG);
    assert_eq!(
        s.step(&[]),
        Err(StepError::PlayerCount {
            expected: 1,
            got: 0
        })
    );
    assert!(matches!(
        s.step(&[cmd(2, 0)]),
        Err(StepError::WrongTick { .. })
    ));
    assert_eq!(s.tick(), Tick::ZERO, "a refused step changes nothing");
    s.step(&[cmd(1, 0)]).unwrap();
    assert_eq!(s.tick(), Tick(1));
}

#[test]
fn events_last_one_tick() {
    let mut s = sim(CONFIG);
    s.step(&[cmd(1, 0)]).unwrap();
    assert_eq!(s.world().events::<Ticked>(), [Ticked(1)]);
    s.step(&[cmd(2, 0)]).unwrap();
    assert_eq!(s.world().events::<Ticked>(), [Ticked(2)]);
}

#[test]
fn same_seed_same_hashes_other_seed_differs() {
    let a = hashes(&mut sim(CONFIG), 120);
    let b = hashes(&mut sim(CONFIG), 120);
    assert_eq!(a, b);
    let c = hashes(&mut sim(SimConfig { seed: 8, ..CONFIG }), 120);
    assert_ne!(a[0], c[0], "the seed is part of the state");
    assert_ne!(a[120], c[120]);
}

#[test]
fn snapshot_restore_resumes_identically() {
    let mut original = sim(CONFIG);
    hashes(&mut original, 50);
    let snapshot = original.snapshot();
    let mut restored = sim(CONFIG);
    restored.restore(&snapshot).unwrap();
    assert_eq!(restored.tick(), Tick(50));
    assert_eq!(hashes(&mut restored, 70), hashes(&mut original, 70));
}

#[test]
fn restore_refuses_other_protocol() {
    let mut b = builder(CONFIG);
    b.add_system(System::new("test.other", Stage::Update, integrate));
    let other = b.build().unwrap();
    let mut s = sim(CONFIG);
    assert!(matches!(
        s.restore(&other.snapshot()),
        Err(RestoreError::Protocol { .. })
    ));
}

#[test]
fn protocol_tracks_schemas_systems_and_tick_rate() {
    let base = sim(CONFIG).protocol_version();
    assert_eq!(
        sim(SimConfig { seed: 1, ..CONFIG }).protocol_version(),
        base
    );
    assert_ne!(
        sim(SimConfig {
            tick_rate: 30,
            ..CONFIG
        })
        .protocol_version(),
        base
    );
    let mut renamed = builder(CONFIG);
    renamed.add_system(System::new("test.integrate2", Stage::Update, integrate));
    assert_ne!(renamed.build().unwrap().protocol_version(), base);
    let mut more = builder(CONFIG);
    more.world_mut().register_component::<Extra>();
    more.add_system(System::new("test.integrate", Stage::Update, integrate));
    assert_ne!(more.build().unwrap().protocol_version(), base);
}

fn poison(w: &mut World) {
    w.resource_mut::<Value>().0 = f32::NAN;
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "[E-NAN] after system test.poison at tick 1")]
fn nan_panics_after_the_system_in_debug() {
    let mut b = builder(CONFIG);
    b.add_system(System::new("test.poison", Stage::Update, poison));
    let mut s = b.build().unwrap();
    let _ = s.step(&[cmd(1, 0)]);
}

#[test]
fn state_hash_names_the_non_finite_field() {
    let mut b = builder(CONFIG);
    b.world_mut().insert_resource(Value(f32::INFINITY));
    let s = b.build().unwrap();
    let err = s.state_hash().unwrap_err();
    assert_eq!(err.path, "/resources/test.Value/0");
}

#[test]
fn state_len_matches_bytes() {
    let s = sim(CONFIG);
    assert_eq!(s.state_len(), s.state_bytes().len());
    assert_eq!(s.snapshot().len(), s.state_len() + 8);
}

#[test]
fn restore_refuses_other_sessions_and_keeps_the_state() {
    let mut other = sim(SimConfig { seed: 99, ..CONFIG });
    hashes(&mut other, 5);
    let mut s = sim(CONFIG);
    hashes(&mut s, 3);
    let before = s.state_bytes();
    assert!(matches!(
        s.restore(&other.snapshot()),
        Err(RestoreError::Session { .. })
    ));
    assert_eq!(s.state_bytes(), before, "a refused restore changes nothing");
}
