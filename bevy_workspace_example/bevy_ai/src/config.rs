use bevy::prelude::*;

pub(crate) const ARENA_HALF_SIZE: Vec2 = Vec2::new(420.0, 300.0);
pub(crate) const PLAYER_SPEED: f32 = 260.0;
pub(crate) const MONSTER_ATTACK_RANGE: f32 = 44.0;
pub(crate) const COMBAT_TICK_SECONDS: f32 = 0.35;
pub(crate) const INVINCIBILITY_SECONDS: f32 = 0.3;
pub(crate) const DAMAGE_FLASH_SECONDS: f32 = 0.12;

pub(crate) fn clamp_to_arena(position: Vec2) -> Vec2 {
  Vec2::new(
    position.x.clamp(-ARENA_HALF_SIZE.x, ARENA_HALF_SIZE.x),
    position.y.clamp(-ARENA_HALF_SIZE.y, ARENA_HALF_SIZE.y),
  )
}
