use bevy::prelude::*;
use lightyear::prelude::{AppComponentExt, NetworkTarget, Replicate};
use serde::{Deserialize, Serialize};

use crate::game::{ActorId, ActorPresentation, ActorType, ArenaPosition, Vitals};

pub(crate) struct GameReplicationPlugin;

impl Plugin for GameReplicationPlugin {
  fn build(&self, app: &mut App) {
    app.component::<ReplicatedActorState>().replicate();
  }
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub(crate) struct ReplicatedActorState {
  pub(crate) id: u64,
  pub(crate) kind: i32,
  pub(crate) x: f32,
  pub(crate) y: f32,
  pub(crate) z: f32,
  pub(crate) red: i32,
  pub(crate) blue: i32,
  pub(crate) animation_phase: f32,
  pub(crate) motion_speed: f32,
  pub(crate) vfx_pulse: u64,
}

impl ReplicatedActorState {
  fn from_actor(
    actor_id: ActorId,
    actor_type: ActorType,
    position: ArenaPosition,
    vitals: Vitals,
    presentation: ActorPresentation,
  ) -> Self {
    Self {
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
}

pub(crate) fn mark_replicated_actors(
  mut commands: Commands,
  actors: Query<
    (
      Entity,
      &ActorId,
      &ActorType,
      &ArenaPosition,
      &Vitals,
      &ActorPresentation,
    ),
    Added<ActorId>,
  >,
) {
  for (entity, actor_id, actor_type, position, vitals, presentation) in &actors {
    commands.entity(entity).insert((
      ReplicatedActorState::from_actor(*actor_id, *actor_type, *position, *vitals, *presentation),
      Replicate::to_clients(NetworkTarget::All),
    ));
  }
}

pub(crate) fn sync_replicated_actor_state(
  mut actors: Query<
    (
      &ActorId,
      &ActorType,
      &ArenaPosition,
      &Vitals,
      &ActorPresentation,
      &mut ReplicatedActorState,
    ),
    Or<(
      Changed<ActorId>,
      Changed<ActorType>,
      Changed<ArenaPosition>,
      Changed<Vitals>,
      Changed<ActorPresentation>,
    )>,
  >,
) {
  for (actor_id, actor_type, position, vitals, presentation, mut replicated_state) in &mut actors {
    *replicated_state =
      ReplicatedActorState::from_actor(*actor_id, *actor_type, *position, *vitals, *presentation);
  }
}
