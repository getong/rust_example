use bevy::prelude::*;
use lightyear::prelude::AppComponentExt;
use serde::{Deserialize, Serialize};

pub struct GameReplicationPlugin;

impl Plugin for GameReplicationPlugin {
  fn build(&self, app: &mut App) {
    app.component::<ReplicatedActorState>().replicate();
  }
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ReplicatedActorState {
  pub id: u64,
  pub kind: i32,
  pub x: f32,
  pub y: f32,
  pub z: f32,
  pub red: i32,
  pub blue: i32,
  pub animation_phase: f32,
  pub motion_speed: f32,
  pub vfx_pulse: u64,
}
