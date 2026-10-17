use bevior_tree::prelude::{BehaviorTreePlugin, BehaviorTreeSystemSet};
use bevy::{ecs::schedule::ApplyDeferred, prelude::*};
use bevy_voxel_world::prelude::VoxelWorldPlugin;

use crate::{
  config::COMBAT_TICK_SECONDS,
  gameplay::{
    CombatClock, despawn_defeated, player_input, resolve_combat, sync_transforms,
    tick_damage_effects,
  },
  levels::{GameLevel, LoadedLevel, advance_when_level_cleared, level_input},
  lighting::animate_sunlight,
  monster_behavior::move_chasing_monsters,
  player_state::transition_player_state,
  setup::{setup_scene, sync_level_world},
  terrain::{GameVoxelWorld, TerrainMap, default_level_map},
  ui::{
    heal_button_interactions, level_button_interactions, update_actor_labels, update_hud,
    update_level_button_styles, update_location_text,
  },
};

pub(crate) struct BevyAiPlugin;

impl Plugin for BevyAiPlugin {
  fn build(&self, app: &mut App) {
    app
      .add_plugins((
        VoxelWorldPlugin::with_config(GameVoxelWorld),
        BehaviorTreePlugin::default().in_schedule(Update),
      ))
      .init_state::<GameLevel>()
      .insert_resource(LoadedLevel::default())
      .insert_resource(default_level_map())
      .insert_resource(TerrainMap::default())
      .insert_resource(CombatClock(Timer::from_seconds(
        COMBAT_TICK_SECONDS,
        TimerMode::Repeating,
      )))
      .add_systems(Startup, setup_scene)
      .add_systems(
        Update,
        sync_level_world.before(BehaviorTreeSystemSet::Update),
      )
      .add_systems(
        Update,
        ApplyDeferred
          .after(sync_level_world)
          .before(BehaviorTreeSystemSet::Update),
      )
      .add_systems(
        Update,
        (
          animate_sunlight,
          level_input,
          player_input
            .after(sync_level_world)
            .before(BehaviorTreeSystemSet::Update),
          move_chasing_monsters
            .after(sync_level_world)
            .after(BehaviorTreeSystemSet::Update),
          resolve_combat.after(move_chasing_monsters),
          transition_player_state
            .after(resolve_combat)
            .after(heal_button_interactions),
          tick_damage_effects.after(resolve_combat),
          despawn_defeated.after(tick_damage_effects),
          advance_when_level_cleared
            .after(sync_level_world)
            .after(despawn_defeated)
            .after(transition_player_state),
          sync_transforms.after(move_chasing_monsters),
          update_actor_labels.after(resolve_combat),
          update_hud
            .after(sync_level_world)
            .after(transition_player_state),
          update_location_text
            .after(sync_level_world)
            .after(transition_player_state),
          level_button_interactions,
          update_level_button_styles,
          heal_button_interactions,
        ),
      );
  }
}
