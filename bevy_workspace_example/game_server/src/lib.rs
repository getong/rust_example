mod behavior;
mod game;
mod net;
pub mod protocol;
mod terrain;

use std::time::Duration;

use bevior_tree::prelude::{BehaviorTreePlugin, BehaviorTreeSystemSet};
use bevy::{app::ScheduleRunnerPlugin, ecs::schedule::ApplyDeferred, log::LogPlugin, prelude::*};

const SERVER_TICK_SECONDS: f64 = 1.0 / 30.0;

pub fn run() {
  App::new()
    .add_plugins((
      MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(
        SERVER_TICK_SECONDS,
      ))),
      LogPlugin::default(),
      BehaviorTreePlugin::default().in_schedule(Update),
    ))
    .init_resource::<game::NextActorId>()
    .init_resource::<game::ServerTick>()
    .init_resource::<game::SnapshotClock>()
    .init_resource::<game::CombatClock>()
    .init_resource::<terrain::LevelMap>()
    .init_resource::<terrain::TerrainMap>()
    .init_resource::<net::Clients>()
    .add_systems(Startup, (net::start_network_server, game::spawn_monsters))
    .add_systems(
      Update,
      (
        net::drain_network_events.before(BehaviorTreeSystemSet::Update),
        ApplyDeferred
          .after(net::drain_network_events)
          .before(BehaviorTreeSystemSet::Update),
        game::apply_player_movement.before(BehaviorTreeSystemSet::Update),
        behavior::move_chasing_monsters.after(BehaviorTreeSystemSet::Update),
        game::resolve_combat.after(behavior::move_chasing_monsters),
        net::broadcast_snapshots.after(game::resolve_combat),
      ),
    )
    .run();
}
