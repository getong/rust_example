use bevior_tree::prelude::BehaviorTreeRoot;
use bevy::{camera::ScalingMode, prelude::*};
use bevy_voxel_world::prelude::VoxelWorldCamera;

use crate::{
  actors::{ActorBundle, ActorKind, Monster, Player, RedBlueValues},
  lighting::{SunPath, initial_sun_transform},
  monster_behavior::monster_behavior_tree,
  terrain::GameVoxelWorld,
  ui::{HudText, spawn_actor_label},
};

pub(crate) fn setup(
  mut commands: Commands,
  mut meshes: ResMut<Assets<Mesh>>,
  mut materials: ResMut<Assets<StandardMaterial>>,
  mut tree_assets: ResMut<Assets<BehaviorTreeRoot>>,
) {
  spawn_camera_and_light(&mut commands);
  let player = spawn_player(&mut commands, &mut meshes, &mut materials);
  spawn_monsters(
    &mut commands,
    &mut meshes,
    &mut materials,
    &mut tree_assets,
    player,
  );
  spawn_hud(&mut commands);
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
) -> Entity {
  let player = commands
    .spawn((
      ActorBundle::new(
        ActorKind::Player,
        Vec2::ZERO,
        RedBlueValues { red: 18, blue: 140 },
      ),
      Mesh3d(meshes.add(Cuboid::new(1.1, 1.4, 1.1))),
      MeshMaterial3d(materials.add(Color::srgb(0.16, 0.38, 1.0))),
      Player,
      Name::new("Player"),
    ))
    .id();
  spawn_actor_label(commands, player, "Player");
  player
}

fn spawn_monsters(
  commands: &mut Commands,
  meshes: &mut Assets<Mesh>,
  materials: &mut Assets<StandardMaterial>,
  tree_assets: &mut Assets<BehaviorTreeRoot>,
  player: Entity,
) {
  let monster_mesh = meshes.add(Cuboid::new(1.0, 1.2, 1.0));

  for (index, position) in [
    Vec2::new(-320.0, 190.0),
    Vec2::new(320.0, 190.0),
    Vec2::new(-300.0, -190.0),
    Vec2::new(310.0, -170.0),
  ]
  .into_iter()
  .enumerate()
  {
    let monster = commands
      .spawn((
        ActorBundle::new(
          ActorKind::Monster,
          position,
          RedBlueValues {
            red: 9 + index as i32,
            blue: 56,
          },
        ),
        Mesh3d(monster_mesh.clone()),
        MeshMaterial3d(materials.add(Color::srgb(0.9, 0.16, 0.12))),
        Monster {
          speed: 92.0 + index as f32 * 10.0,
        },
        monster_behavior_tree(player, tree_assets),
        Name::new(format!("Monster {}", index + 1)),
      ))
      .id();
    spawn_actor_label(commands, monster, "Monster");
  }
}

fn spawn_hud(commands: &mut Commands) {
  commands.spawn((
    Text::new("WASD: move | Red: attack | Blue: health"),
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
