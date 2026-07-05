use bevy::prelude::*;

use crate::{
  actors::{ArenaPosition, Monster, Player, RedBlueValues},
  config::{MONSTER_ATTACK_RANGE, PLAYER_SPEED},
  terrain::{TerrainMap, game_to_world_position},
};

#[derive(Resource)]
pub(crate) struct CombatClock(pub(crate) Timer);

pub(crate) fn player_input(
  keyboard: Res<ButtonInput<KeyCode>>,
  time: Res<Time>,
  terrain_map: Res<TerrainMap>,
  mut players: Query<(&mut ArenaPosition, &RedBlueValues), With<Player>>,
) {
  let mut direction = Vec2::ZERO;

  if keyboard.pressed(KeyCode::KeyW) {
    direction.y -= 1.0;
  }
  if keyboard.pressed(KeyCode::KeyS) {
    direction.y += 1.0;
  }
  if keyboard.pressed(KeyCode::KeyA) {
    direction.x -= 1.0;
  }
  if keyboard.pressed(KeyCode::KeyD) {
    direction.x += 1.0;
  }

  if direction == Vec2::ZERO {
    return;
  }

  for (mut position, values) in &mut players {
    if values.blue <= 0 {
      continue;
    }

    let movement = direction.normalize() * PLAYER_SPEED * time.delta_secs();
    position.0 = terrain_map.try_move(position.0, movement);
  }
}

pub(crate) fn monster_ai(
  time: Res<Time>,
  terrain_map: Res<TerrainMap>,
  player_query: Query<(&ArenaPosition, &RedBlueValues), With<Player>>,
  mut monsters: Query<(&mut ArenaPosition, &Monster, &RedBlueValues), Without<Player>>,
) {
  let Ok((player_position, player_values)) = player_query.single() else {
    return;
  };
  if player_values.blue <= 0 {
    return;
  }

  for (mut monster_position, monster, values) in &mut monsters {
    if values.blue <= 0 {
      continue;
    }

    let to_player = player_position.0 - monster_position.0;
    if to_player.length() <= MONSTER_ATTACK_RANGE
      && terrain_map.segment_is_walkable(monster_position.0, player_position.0)
    {
      continue;
    }

    let waypoint = terrain_map.next_waypoint(monster_position.0, player_position.0);
    let to_waypoint = waypoint - monster_position.0;
    if to_waypoint.length() <= 1.0 {
      continue;
    }

    let movement = to_waypoint.normalize_or_zero() * monster.speed * time.delta_secs();
    monster_position.0 = terrain_map.try_move(monster_position.0, movement);
  }
}

pub(crate) fn resolve_combat(
  time: Res<Time>,
  mut clock: ResMut<CombatClock>,
  terrain_map: Res<TerrainMap>,
  mut player_query: Query<(&ArenaPosition, &mut RedBlueValues), (With<Player>, Without<Monster>)>,
  mut monsters: Query<(&ArenaPosition, &mut RedBlueValues), (With<Monster>, Without<Player>)>,
) {
  clock.0.tick(time.delta());
  if !clock.0.just_finished() {
    return;
  }

  let Ok((player_position, mut player_values)) = player_query.single_mut() else {
    return;
  };
  if player_values.blue <= 0 {
    return;
  }

  let mut incoming_damage = 0;

  for (monster_position, mut monster_values) in &mut monsters {
    if monster_values.blue <= 0 {
      continue;
    }

    let distance = player_position.0.distance(monster_position.0);
    if distance <= MONSTER_ATTACK_RANGE
      && terrain_map.segment_is_walkable(player_position.0, monster_position.0)
    {
      monster_values.blue = (monster_values.blue - player_values.red).max(0);
      if monster_values.blue > 0 {
        incoming_damage += monster_values.red;
      }
    }
  }

  player_values.blue = (player_values.blue - incoming_damage).max(0);
}

pub(crate) fn despawn_defeated(
  mut commands: Commands,
  defeated_monsters: Query<(Entity, &RedBlueValues), With<Monster>>,
) {
  for (entity, values) in &defeated_monsters {
    if values.blue <= 0 {
      commands.entity(entity).despawn();
    }
  }
}

pub(crate) fn sync_transforms(
  mut materials: ResMut<Assets<StandardMaterial>>,
  mut actors: Query<(
    &ArenaPosition,
    &RedBlueValues,
    &MeshMaterial3d<StandardMaterial>,
    &mut Transform,
  )>,
) {
  for (position, values, material, mut transform) in &mut actors {
    transform.translation = game_to_world_position(position.0, 1.0);

    let health_ratio = (values.blue.max(0) as f32 / 140.0).clamp(0.15, 1.0);
    if let Some(mut material) = materials.get_mut(&material.0) {
      material.base_color = Color::srgb(
        health_ratio,
        (values.red as f32 / 24.0).clamp(0.1, 0.85),
        1.0 - health_ratio * 0.55,
      );
    }
  }
}
