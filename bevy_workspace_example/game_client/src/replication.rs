use bevy::prelude::*;
use lightyear::prelude::AppComponentExt;
use serde::{Deserialize, Serialize};

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
