use bevior_tree::prelude::{BehaviorTree, BehaviorTreeRoot};
use bevy::prelude::*;

use crate::{
  behavior::monster_behavior_tree,
  protocol::{ActorKind, ActorState},
  terrain::{LevelMap, TerrainMap, clamp_to_playable_area},
};

pub(crate) const ARENA_HALF_SIZE: Vec2 = Vec2::new(420.0, 300.0);
pub(crate) const PLAYER_SPEED: f32 = 260.0;
pub(crate) const MONSTER_ATTACK_RANGE: f32 = 44.0;
const SNAPSHOT_SECONDS: f32 = 0.1;
const COMBAT_TICK_SECONDS: f32 = 0.35;
const MONSTER_DAMAGE: i32 = 8;

#[derive(Resource, Debug)]
pub(crate) struct NextActorId(u64);

impl Default for NextActorId {
  fn default() -> Self {
    Self(1)
  }
}

impl NextActorId {
  pub(crate) fn next(&mut self) -> ActorId {
    let id = self.0;
    self.0 += 1;
    ActorId(id)
  }
}

#[derive(Resource, Debug, Default)]
pub(crate) struct ServerTick(pub(crate) u64);

#[derive(Resource, Debug)]
pub(crate) struct SnapshotClock(pub(crate) Timer);

impl Default for SnapshotClock {
  fn default() -> Self {
    Self(Timer::from_seconds(SNAPSHOT_SECONDS, TimerMode::Repeating))
  }
}

#[derive(Resource, Debug)]
pub(crate) struct CombatClock(pub(crate) Timer);

impl Default for CombatClock {
  fn default() -> Self {
    Self(Timer::from_seconds(
      COMBAT_TICK_SECONDS,
      TimerMode::Repeating,
    ))
  }
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ActorId(pub(crate) u64);

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ActorType(pub(crate) ActorKind);

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub(crate) struct ArenaPosition(pub(crate) Vec2);

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Vitals {
  pub(crate) red: i32,
  pub(crate) blue: i32,
}

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub(crate) struct PlayerInputState {
  pub(crate) direction: Vec2,
}

impl Default for PlayerInputState {
  fn default() -> Self {
    Self {
      direction: Vec2::ZERO,
    }
  }
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Player {
  pub(crate) client_id: u64,
}

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub(crate) struct Monster {
  pub(crate) speed: f32,
}

pub(crate) fn spawn_monsters(
  mut commands: Commands,
  mut actor_ids: ResMut<NextActorId>,
  mut tree_assets: ResMut<Assets<BehaviorTreeRoot>>,
  level_map: Res<LevelMap>,
) {
  for monster in &level_map.monsters {
    spawn_monster(
      &mut commands,
      actor_ids.next(),
      monster.position,
      Vitals {
        red: monster.red,
        blue: monster.blue,
      },
      monster.speed,
      monster_behavior_tree(&mut tree_assets),
    );
  }
}

pub(crate) fn spawn_player(
  commands: &mut Commands,
  actor_id: ActorId,
  client_id: u64,
  connected_players: usize,
  level_map: &LevelMap,
) -> Entity {
  let offset = connected_players as f32 * 56.0;
  commands
    .spawn((
      actor_id,
      ActorType(ActorKind::Player),
      ArenaPosition(clamp_to_playable_area(
        level_map.player_spawn + Vec2::new(offset, 0.0),
      )),
      Vitals { red: 18, blue: 140 },
      PlayerInputState::default(),
      Player { client_id },
    ))
    .id()
}

pub(crate) fn apply_player_movement(
  time: Res<Time>,
  terrain_map: Res<TerrainMap>,
  mut server_tick: ResMut<ServerTick>,
  mut players: Query<(&mut ArenaPosition, &PlayerInputState, &Vitals), With<Player>>,
) {
  server_tick.0 += 1;

  for (mut position, input, vitals) in &mut players {
    if vitals.blue <= 0 {
      continue;
    }

    let direction = input.direction.normalize_or_zero();
    let movement = direction * PLAYER_SPEED * time.delta_secs();
    position.0 = terrain_map.try_move(position.0, movement);
  }
}

pub(crate) fn resolve_combat(
  time: Res<Time>,
  terrain_map: Res<TerrainMap>,
  mut combat_clock: ResMut<CombatClock>,
  monsters: Query<(&ArenaPosition, &Vitals), With<Monster>>,
  mut players: Query<(&ArenaPosition, &mut Vitals), (With<Player>, Without<Monster>)>,
) {
  combat_clock.0.tick(time.delta());
  if !combat_clock.0.just_finished() {
    return;
  }

  for (player_position, mut player_vitals) in &mut players {
    if player_vitals.blue <= 0 {
      continue;
    }

    let attackers = monsters
      .iter()
      .filter(|(monster_position, monster_vitals)| {
        monster_vitals.blue > 0
          && monster_position.0.distance(player_position.0) <= MONSTER_ATTACK_RANGE
          && terrain_map.segment_is_walkable(monster_position.0, player_position.0)
      })
      .count() as i32;

    if attackers > 0 {
      player_vitals.blue = (player_vitals.blue - attackers * MONSTER_DAMAGE).max(0);
    }
  }
}

pub(crate) fn actor_state(
  actor_id: ActorId,
  actor_type: ActorType,
  position: ArenaPosition,
  vitals: Vitals,
) -> ActorState {
  ActorState {
    id: actor_id.0,
    kind: actor_type.0 as i32,
    x: position.0.x,
    y: position.0.y,
    red: vitals.red,
    blue: vitals.blue,
  }
}

fn spawn_monster(
  commands: &mut Commands,
  actor_id: ActorId,
  position: Vec2,
  vitals: Vitals,
  speed: f32,
  behavior_tree: BehaviorTree,
) {
  commands.spawn((
    actor_id,
    ActorType(ActorKind::Monster),
    ArenaPosition(clamp_to_playable_area(position)),
    vitals,
    Monster { speed },
    behavior_tree,
  ));
}
