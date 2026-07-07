use bevy::prelude::*;

use crate::{
  actors::{ArenaPosition, DamageFlash, InvincibilityTimer, Monster, Player, RedBlueValues},
  config::{DAMAGE_FLASH_SECONDS, INVINCIBILITY_SECONDS, MONSTER_ATTACK_RANGE, PLAYER_SPEED},
  player_state::PlayerActive,
  terrain::{TerrainMap, game_to_world_position},
};

#[derive(Resource)]
pub(crate) struct CombatClock(pub(crate) Timer);

pub(crate) fn player_input(
  keyboard: Res<ButtonInput<KeyCode>>,
  time: Res<Time>,
  terrain_map: Res<TerrainMap>,
  mut players: Query<&mut ArenaPosition, (With<Player>, With<PlayerActive>)>,
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

  for mut position in &mut players {
    let movement = direction.normalize() * PLAYER_SPEED * time.delta_secs();
    position.0 = terrain_map.try_move(position.0, movement);
  }
}

pub(crate) fn resolve_combat(
  mut commands: Commands,
  time: Res<Time>,
  mut clock: ResMut<CombatClock>,
  terrain_map: Res<TerrainMap>,
  mut player_query: Query<
    (
      Entity,
      &ArenaPosition,
      &mut RedBlueValues,
      Option<&InvincibilityTimer>,
    ),
    (With<Player>, Without<Monster>),
  >,
  mut monsters: Query<
    (Entity, &ArenaPosition, &mut RedBlueValues),
    (With<Monster>, Without<Player>),
  >,
) {
  clock.0.tick(time.delta());
  if !clock.0.just_finished() {
    return;
  }

  let Ok((player_entity, player_position, mut player_values, invincibility)) =
    player_query.single_mut()
  else {
    return;
  };
  if player_values.blue <= 0 {
    return;
  }

  let mut incoming_damage = 0;

  for (monster_entity, monster_position, mut monster_values) in &mut monsters {
    if monster_values.blue <= 0 {
      continue;
    }

    let distance = player_position.0.distance(monster_position.0);
    if distance <= MONSTER_ATTACK_RANGE
      && terrain_map.segment_is_walkable(player_position.0, monster_position.0)
    {
      let damage = player_values.red.min(monster_values.blue);
      monster_values.blue = (monster_values.blue - player_values.red).max(0);
      if damage > 0 {
        commands
          .entity(monster_entity)
          .insert(DamageFlash(Timer::from_seconds(
            DAMAGE_FLASH_SECONDS,
            TimerMode::Once,
          )));
      }
      if monster_values.blue > 0 {
        incoming_damage += monster_values.red;
      }
    }
  }

  if incoming_damage > 0 && invincibility.is_none() {
    let damage = incoming_damage.min(player_values.blue);
    player_values.blue = (player_values.blue - incoming_damage).max(0);
    if damage > 0 {
      commands.entity(player_entity).insert((
        DamageFlash(Timer::from_seconds(DAMAGE_FLASH_SECONDS, TimerMode::Once)),
        InvincibilityTimer(Timer::from_seconds(INVINCIBILITY_SECONDS, TimerMode::Once)),
      ));
    }
  }
}

pub(crate) fn tick_damage_effects(
  mut commands: Commands,
  time: Res<Time>,
  mut flash_q: Query<(Entity, &mut DamageFlash)>,
  mut invincibility_q: Query<(Entity, &mut InvincibilityTimer)>,
) {
  for (entity, mut flash) in &mut flash_q {
    flash.0.tick(time.delta());
    if flash.0.just_finished() {
      commands.entity(entity).remove::<DamageFlash>();
    }
  }
  for (entity, mut inv) in &mut invincibility_q {
    inv.0.tick(time.delta());
    if inv.0.just_finished() {
      commands.entity(entity).remove::<InvincibilityTimer>();
    }
  }
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
    Option<&DamageFlash>,
  )>,
) {
  for (position, values, material, mut transform, flash) in &mut actors {
    transform.translation = game_to_world_position(position.0, 1.0);

    if let Some(mut mat) = materials.get_mut(&material.0) {
      if flash.is_some() {
        mat.base_color = Color::srgb(1.0, 0.15, 0.15);
      } else {
        let health_ratio = (values.blue.max(0) as f32 / 140.0).clamp(0.15, 1.0);
        mat.base_color = Color::srgb(
          health_ratio,
          (values.red as f32 / 24.0).clamp(0.1, 0.85),
          1.0 - health_ratio * 0.55,
        );
      }
    }
  }
}
