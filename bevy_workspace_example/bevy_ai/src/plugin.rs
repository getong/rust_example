use bevior_tree::prelude::{BehaviorTreePlugin, BehaviorTreeSystemSet};
use bevy::{ecs::schedule::ApplyDeferred, prelude::*};
use bevy_voxel_world::prelude::VoxelWorldPlugin;
use seldom_state::{prelude::StateMachinePlugin, set::StateSet};

use crate::{
  config::COMBAT_TICK_SECONDS,
  gameplay::{
    CombatClock, despawn_defeated, player_input, resolve_combat, sync_transforms,
    tick_damage_effects,
  },
  levels::{GameLevel, LoadedLevel, advance_when_level_cleared, level_input},
  lighting::animate_sunlight,
  monster_behavior::move_chasing_monsters,
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
        StateMachinePlugin::default().schedule(Update),
      ))
      .init_state::<GameLevel>()
      .insert_resource(LoadedLevel::default())
      .insert_resource(default_level_map())
      .configure_sets(Update, StateSet::Transition.after(resolve_combat))
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
          tick_damage_effects.after(resolve_combat),
          despawn_defeated.after(tick_damage_effects),
          advance_when_level_cleared
            .after(sync_level_world)
            .after(despawn_defeated),
          sync_transforms.after(move_chasing_monsters),
          update_actor_labels.after(resolve_combat),
          update_hud
            .after(sync_level_world)
            .after(StateSet::Transition),
          update_location_text
            .after(sync_level_world)
            .after(StateSet::Transition),
          level_button_interactions,
          update_level_button_styles,
          heal_button_interactions,
        ),
      );
  }
}
