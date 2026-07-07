use bevior_tree::prelude::{BehaviorTreePlugin, BehaviorTreeSystemSet};
use bevy::prelude::*;
use bevy_voxel_world::prelude::VoxelWorldPlugin;

use crate::{
  config::COMBAT_TICK_SECONDS,
  gameplay::{CombatClock, despawn_defeated, player_input, resolve_combat, sync_transforms},
  monster_behavior::move_chasing_monsters,
  setup::setup,
  terrain::{GameVoxelWorld, TerrainMap},
  ui::{update_actor_labels, update_hud},
};

pub(crate) struct BevyAiPlugin;

impl Plugin for BevyAiPlugin {
  fn build(&self, app: &mut App) {
    app
      .add_plugins((
        VoxelWorldPlugin::with_config(GameVoxelWorld),
        BehaviorTreePlugin::default().in_schedule(Update),
      ))
      .insert_resource(TerrainMap::default())
      .insert_resource(CombatClock(Timer::from_seconds(
        COMBAT_TICK_SECONDS,
        TimerMode::Repeating,
      )))
      .add_systems(Startup, setup)
      .add_systems(
        Update,
        (
          player_input.before(BehaviorTreeSystemSet::Update),
          move_chasing_monsters.after(BehaviorTreeSystemSet::Update),
          resolve_combat.after(move_chasing_monsters),
          despawn_defeated.after(resolve_combat),
          sync_transforms.after(move_chasing_monsters),
          update_actor_labels.after(resolve_combat),
          update_hud.after(resolve_combat),
        ),
      );
  }
}
