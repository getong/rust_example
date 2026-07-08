use bevy::prelude::*;
pub(crate) use game_shared::replication::GameReplicationPlugin;
use game_shared::replication::ReplicatedActorState;
use lightyear::prelude::{NetworkTarget, Replicate};

use crate::game::{ActorId, ActorPresentation, ActorType, ArenaPosition, Vitals};

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
      replicated_actor_state(*actor_id, *actor_type, *position, *vitals, *presentation),
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
      replicated_actor_state(*actor_id, *actor_type, *position, *vitals, *presentation);
  }
}

fn replicated_actor_state(
  actor_id: ActorId,
  actor_type: ActorType,
  position: ArenaPosition,
  vitals: Vitals,
  presentation: ActorPresentation,
) -> ReplicatedActorState {
  ReplicatedActorState {
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
