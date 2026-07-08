use avian3d::prelude::{Collider, LinearVelocity, Position, RigidBody};
use bevior_tree::prelude::{BehaviorTree, BehaviorTreeRoot};
use bevy::prelude::*;

use crate::{
  behavior::monster_behavior_tree,
  protocol::{ActorKind, ActorState},
  terrain::{LevelMap, TerrainMap, clamp_to_playable_area},
};

pub(crate) const ARENA_HALF_EXTENTS: Vec3 = Vec3::new(420.0, 0.5, 300.0);
pub(crate) const PLAYER_SIZE: Vec3 = Vec3::new(28.0, 36.0, 28.0);
pub(crate) const MONSTER_SIZE: Vec3 = Vec3::new(24.0, 32.0, 24.0);
pub(crate) const PLAYER_CENTER_Y: f32 = PLAYER_SIZE.y * 0.5;
pub(crate) const MONSTER_CENTER_Y: f32 = MONSTER_SIZE.y * 0.5;
pub(crate) const PLAYER_SPEED: f32 = 260.0;
pub(crate) const MONSTER_ATTACK_RANGE: f32 = 44.0;
const SNAPSHOT_SECONDS: f32 = 0.1;
const COMBAT_TICK_SECONDS: f32 = 0.35;
const MONSTER_DAMAGE: i32 = 8;
const ANIMATION_STRIDE_LENGTH: f32 = 80.0;

#[derive(Resource, Debug)]
pub(crate) struct NextActorId(u64);

impl Default for NextActorId {
  fn default() -> Self {
    Self(1)
  }
}

impl NextActorId {
  pub(crate) fn new(first_id: u64) -> Self {
    Self(first_id.max(1))
  }

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
pub(crate) struct ArenaPosition(pub(crate) Vec3);

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub(crate) struct PreviousArenaPosition(pub(crate) Vec3);

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub(crate) struct ActorPresentation {
  pub(crate) animation_phase: f32,
  pub(crate) motion_speed: f32,
  pub(crate) vfx_pulse: u64,
}

impl ActorPresentation {
  fn spawned() -> Self {
    Self {
      animation_phase: 0.0,
      motion_speed: 0.0,
      vfx_pulse: 1,
    }
  }
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Vitals {
  pub(crate) red: i32,
  pub(crate) blue: i32,
}

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub(crate) struct PlayerInputState {
  pub(crate) direction: Vec3,
}

impl Default for PlayerInputState {
  fn default() -> Self {
    Self {
      direction: Vec3::ZERO,
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
  let position = clamp_to_playable_area(level_map.player_spawn + Vec3::new(offset, 0.0, 0.0));
  commands
    .spawn((
      actor_id,
      ActorType(ActorKind::Player),
      ArenaPosition(position),
      PreviousArenaPosition(position),
      ActorPresentation::spawned(),
      Transform::from_translation(position),
      Position::new(position),
      RigidBody::Kinematic,
      Collider::cuboid(PLAYER_SIZE.x, PLAYER_SIZE.y, PLAYER_SIZE.z),
      LinearVelocity::ZERO,
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

    let direction = Vec3::new(input.direction.x, 0.0, input.direction.z).normalize_or_zero();
    let movement = direction * PLAYER_SPEED * time.delta_secs();
    position.0 = terrain_map.try_move(position.0, movement);
  }
}

pub(crate) fn update_actor_presentation(
  time: Res<Time>,
  mut actors: Query<(
    &ArenaPosition,
    &mut PreviousArenaPosition,
    &mut ActorPresentation,
  )>,
) {
  let delta_secs = time.delta_secs().max(f32::EPSILON);
  for (position, mut previous_position, mut presentation) in &mut actors {
    let distance = horizontal_distance(previous_position.0, position.0);
    presentation.motion_speed = distance / delta_secs;
    presentation.animation_phase =
      (presentation.animation_phase + distance / ANIMATION_STRIDE_LENGTH).fract();
    previous_position.0 = position.0;
  }
}

pub(crate) fn sync_actor_transforms(
  mut actors: Query<(&ArenaPosition, &mut Transform, Option<&mut Position>)>,
) {
  for (position, mut transform, physics_position) in &mut actors {
    transform.translation = position.0;
    if let Some(mut physics_position) = physics_position {
      physics_position.0 = position.0;
    }
  }
}

pub(crate) fn resolve_combat(
  time: Res<Time>,
  terrain_map: Res<TerrainMap>,
  mut combat_clock: ResMut<CombatClock>,
  monsters: Query<(&ArenaPosition, &Vitals), With<Monster>>,
  mut players: Query<
    (&ArenaPosition, &mut Vitals, &mut ActorPresentation),
    (With<Player>, Without<Monster>),
  >,
) {
  combat_clock.0.tick(time.delta());
  if !combat_clock.0.just_finished() {
    return;
  }

  for (player_position, mut player_vitals, mut presentation) in &mut players {
    if player_vitals.blue <= 0 {
      continue;
    }

    let attackers = monsters
      .iter()
      .filter(|(monster_position, monster_vitals)| {
        monster_vitals.blue > 0
          && horizontal_distance(monster_position.0, player_position.0) <= MONSTER_ATTACK_RANGE
          && terrain_map.segment_is_walkable(monster_position.0, player_position.0)
      })
      .count() as i32;

    if attackers > 0 {
      let previous_blue = player_vitals.blue;
      player_vitals.blue = (player_vitals.blue - attackers * MONSTER_DAMAGE).max(0);
      if player_vitals.blue < previous_blue {
        presentation.vfx_pulse = presentation.vfx_pulse.saturating_add(1);
      }
    }
  }
}

pub(crate) fn actor_state(
  actor_id: ActorId,
  actor_type: ActorType,
  position: ArenaPosition,
  vitals: Vitals,
  presentation: ActorPresentation,
) -> ActorState {
  ActorState {
    id: actor_id.0,
    kind: actor_type.0 as i32,
    x: position.0.x,
    y: position.0.y,
    z: position.0.z,
    red: vitals.red,
    blue: vitals.blue,
    animation_phase: presentation.animation_phase,
    motion_speed: presentation.motion_speed,
    vfx_pulse: presentation.vfx_pulse,
  }
}

fn spawn_monster(
  commands: &mut Commands,
  actor_id: ActorId,
  position: Vec3,
  vitals: Vitals,
  speed: f32,
  behavior_tree: BehaviorTree,
) {
  let position = clamp_to_playable_area(position);
  commands.spawn((
    actor_id,
    ActorType(ActorKind::Monster),
    ArenaPosition(position),
    PreviousArenaPosition(position),
    ActorPresentation::spawned(),
    Transform::from_translation(position),
    Position::new(position),
    RigidBody::Kinematic,
    Collider::cuboid(MONSTER_SIZE.x, MONSTER_SIZE.y, MONSTER_SIZE.z),
    LinearVelocity::ZERO,
    vitals,
    Monster { speed },
    behavior_tree,
  ));
}

fn horizontal_distance(from: Vec3, to: Vec3) -> f32 {
  Vec2::new(to.x - from.x, to.z - from.z).length()
}
