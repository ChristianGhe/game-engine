use eng_core::{PlayerCommand, Tick};
use eng_sim::{Sim, SimConfig};
use game_testbed_sim::{TICK_RATE, WallBump, build};
use kit_fps::MoveState;

fn config(seed: u64, players: u8) -> SimConfig {
    SimConfig {
        tick_rate: TICK_RATE,
        players,
        seed,
    }
}

fn step_all(sim: &mut Sim, forward: i8, yaw: u16) {
    let tick = sim.tick().next();
    let cmds: Vec<PlayerCommand> = (0..sim.config().players)
        .map(|_| PlayerCommand {
            forward,
            view_yaw: yaw,
            ..PlayerCommand::idle(tick)
        })
        .collect();
    sim.step(&cmds).unwrap();
}

#[test]
fn dots_spawn_inside_the_room_from_the_seed() {
    let a = build(config(1, 2)).unwrap();
    let b = build(config(1, 2)).unwrap();
    let c = build(config(2, 2)).unwrap();
    assert_eq!(a.state_hash(), b.state_hash());
    assert_ne!(a.state_hash(), c.state_hash());
    assert_eq!(a.world().storage::<MoveState>().len(), 2);
}

#[test]
fn running_east_hits_the_wall_and_reports_it() {
    let mut sim = build(config(1, 1)).unwrap();
    let mut bumps = 0;
    for _ in 0..180 {
        step_all(&mut sim, 127, 0);
        bumps += sim.world().events::<WallBump>().len();
    }
    let (_, state) = sim.world().storage::<MoveState>().iter().next().unwrap();
    assert_eq!(state.pos.x, 7.75);
    assert!(bumps > 0);
    assert_eq!(sim.tick(), Tick(180));
}

#[test]
fn system_order_is_stable() {
    let sim = build(config(1, 1)).unwrap();
    let order: Vec<_> = sim.system_order().map(|(s, n)| (s.name(), n)).collect();
    assert_eq!(order, [("Update", "testbed.move_dots")]);
}
