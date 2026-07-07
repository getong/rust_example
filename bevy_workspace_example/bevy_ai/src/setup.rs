use bevior_tree::prelude::BehaviorTreeRoot;
use bevy::{camera::ScalingMode, prelude::*};
use bevy_state::prelude::{DespawnOnExit, State};
use bevy_voxel_world::prelude::VoxelWorldCamera;

use crate::{
  actors::{ActorBundle, ActorKind, MaxHealth, Monster, Player, RedBlueValues},
  gameplay::CombatClock,
  levels::{GameLevel, LevelEntity, LoadedLevel, level_number, level_total},
  lighting::{SunPath, initial_sun_transform},
  monster_behavior::monster_behavior_tree,
  player_state::{PlayerActive, player_state_machine},
  terrain::{GameVoxelWorld, LevelMap, TerrainMap, write_level_voxels},
  ui::{HudText, spawn_actor_label, spawn_heal_button, spawn_level_buttons, spawn_location_text},
};

pub(crate) fn setup_scene(mut commands: Commands) {
  spawn_camera_and_light(&mut commands);
  spawn_hud(&mut commands);
  spawn_level_buttons(&mut commands);
  spawn_location_text(&mut commands);
  spawn_heal_button(&mut commands);
}

pub(crate) fn sync_level_world(
  mut commands: Commands,
  level: Res<State<GameLevel>>,
  mut loaded_level: ResMut<LoadedLevel>,
  mut current_map: ResMut<LevelMap>,
  mut terrain_map: ResMut<TerrainMap>,
  level_entities: Query<Entity, With<LevelEntity>>,
  mut meshes: ResMut<Assets<Mesh>>,
  mut materials: ResMut<Assets<StandardMaterial>>,
  mut tree_assets: ResMut<Assets<BehaviorTreeRoot>>,
  voxel_world: Option<bevy_voxel_world::prelude::VoxelWorld<GameVoxelWorld>>,
  mut combat_clock: ResMut<CombatClock>,
) {
  let Some(mut voxel_world) = voxel_world else {
    return;
  };

  let level = *level.get();
  if loaded_level.current == Some(level) {
    return;
  }

  for entity in &level_entities {
    commands.entity(entity).despawn();
  }

  let map = level.map();
  write_level_voxels(&mut voxel_world, &map);
  combat_clock.0.reset();

  *current_map = map.clone();
  *terrain_map = TerrainMap::for_level(map.clone());
  loaded_level.current = Some(level);

  let player = spawn_player(&mut commands, &mut meshes, &mut materials, level, &map);
  spawn_monsters(
    &mut commands,
    &mut meshes,
    &mut materials,
    &mut tree_assets,
    player,
    level,
    &map,
  );

  info!(
    "Loaded L{} {} with {} roadblocks",
    level_number(level),
    map.name,
    map.obstacles.len()
  );
}

fn spawn_camera_and_light(commands: &mut Commands) {
  commands.spawn((
    Camera3d::default(),
    Projection::from(OrthographicProjection {
      scaling_mode: ScalingMode::Fixed {
        width: 42.0,
        height: 28.0,
      },
      ..OrthographicProjection::default_3d()
    }),
    Transform::from_xyz(0.0, 28.887, 12.243).looking_at(Vec3::new(0.0, 1.5, 0.0), Vec3::Y),
    VoxelWorldCamera::<GameVoxelWorld>::default(),
    IsDefaultUiCamera,
    AmbientLight {
      brightness: 60.0,
      ..default()
    },
  ));

  commands.spawn((
    DirectionalLight {
      illuminance: 18_000.0,
      shadow_maps_enabled: true,
      ..default()
    },
    initial_sun_transform(),
    SunPath,
  ));
}

fn spawn_player(
  commands: &mut Commands,
  meshes: &mut Assets<Mesh>,
  materials: &mut Assets<StandardMaterial>,
  level: GameLevel,
  map: &LevelMap,
) -> Entity {
  let player = commands
    .spawn((
      ActorBundle::new(
        ActorKind::Player,
        map.player_spawn,
        RedBlueValues { red: 18, blue: 140 },
      ),
      Mesh3d(meshes.add(Cuboid::new(1.1, 1.4, 1.1))),
      MeshMaterial3d(materials.add(Color::srgb(0.16, 0.38, 1.0))),
      Player,
      PlayerActive,
      MaxHealth(140),
      player_state_machine(),
      LevelEntity,
      DespawnOnExit(level),
      Name::new("Player"),
    ))
    .id();
  spawn_actor_label(commands, player, "Player", level);
  player
}

fn spawn_monsters(
  commands: &mut Commands,
  meshes: &mut Assets<Mesh>,
  materials: &mut Assets<StandardMaterial>,
  tree_assets: &mut Assets<BehaviorTreeRoot>,
  player: Entity,
  level: GameLevel,
  map: &LevelMap,
) {
  let monster_mesh = meshes.add(Cuboid::new(1.0, 1.2, 1.0));

  for (index, spawn) in map.monsters.iter().enumerate() {
    let monster = commands
      .spawn((
        ActorBundle::new(
          ActorKind::Monster,
          spawn.position,
          RedBlueValues {
            red: spawn.red,
            blue: spawn.blue,
          },
        ),
        Mesh3d(monster_mesh.clone()),
        MeshMaterial3d(materials.add(Color::srgb(0.9, 0.16, 0.12))),
        Monster { speed: spawn.speed },
        MaxHealth(spawn.blue),
        monster_behavior_tree(player, tree_assets),
        LevelEntity,
        DespawnOnExit(level),
        Name::new(format!("Monster {}", index + 1)),
      ))
      .id();
    spawn_actor_label(commands, monster, "Monster", level);
  }
}

fn spawn_hud(commands: &mut Commands) {
  let help = format!("WASD: move | 1-{}: level | Heal", level_total());
  commands.spawn((
    Text::new(help),
    TextFont {
      font_size: FontSize::Px(22.0),
      ..default()
    },
    TextColor(Color::WHITE),
    TextShadow::default(),
    Node {
      position_type: PositionType::Absolute,
      top: px(14.0),
      left: px(16.0),
      ..default()
    },
    HudText,
  ));
}
