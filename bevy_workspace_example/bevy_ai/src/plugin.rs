use bevy::prelude::*;
use bevy_voxel_world::prelude::VoxelWorldPlugin;

use crate::{
  config::COMBAT_TICK_SECONDS,
  gameplay::{
    CombatClock, despawn_defeated, monster_ai, player_input, resolve_combat, sync_transforms,
  },
  setup::setup,
  terrain::{GameVoxelWorld, TerrainMap},
  ui::{update_actor_labels, update_hud},
};

pub(crate) struct BevyAiPlugin;

impl Plugin for BevyAiPlugin {
  fn build(&self, app: &mut App) {
    app
      .add_plugins(VoxelWorldPlugin::with_config(GameVoxelWorld))
      .insert_resource(TerrainMap::default())
      .insert_resource(CombatClock(Timer::from_seconds(
        COMBAT_TICK_SECONDS,
        TimerMode::Repeating,
      )))
      .add_systems(Startup, setup)
      .add_systems(
        Update,
        (
          player_input,
          monster_ai.after(player_input),
          resolve_combat.after(monster_ai),
          despawn_defeated.after(resolve_combat),
          sync_transforms,
          update_actor_labels.after(resolve_combat),
          update_hud.after(resolve_combat),
        ),
      );
  }
}
