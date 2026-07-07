use bevy::prelude::*;

use crate::terrain::{clamp_to_playable_area, game_to_world_position};

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub(crate) struct ArenaPosition(pub(crate) Vec2);

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RedBlueValues {
  pub(crate) red: i32,
  pub(crate) blue: i32,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MaxHealth(pub(crate) i32);

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActorKind {
  Player,
  Monster,
}

#[derive(Component)]
pub(crate) struct Player;

#[derive(Component)]
pub(crate) struct Monster {
  pub(crate) speed: f32,
}

#[derive(Component)]
pub(crate) struct DamageFlash(pub(crate) Timer);

#[derive(Component)]
pub(crate) struct InvincibilityTimer(pub(crate) Timer);

#[derive(Bundle)]
pub(crate) struct ActorBundle {
  kind: ActorKind,
  position: ArenaPosition,
  values: RedBlueValues,
  transform: Transform,
}

impl ActorBundle {
  pub(crate) fn new(kind: ActorKind, position: Vec2, values: RedBlueValues) -> Self {
    let position = clamp_to_playable_area(position);

    Self {
      kind,
      position: ArenaPosition(position),
      values,
      transform: Transform::from_translation(game_to_world_position(position, 1.0)),
    }
  }
}
