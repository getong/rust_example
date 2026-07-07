//! A small navigation stack demo based on the article's
//! Avian -> oxidized_navigation -> landmass pipeline.
//!
//! The demo is a standalone crate because `oxidized_navigation 0.12` and
//! `landmass_oxidized_navigation 0.2` are Bevy 0.15 integrations, while the
//! main game in this repository is already on Bevy 0.19.

use avian3d::prelude::{Collider, PhysicsPlugins};
use bevy::{
  color::palettes::css::{DARK_GRAY, MEDIUM_SEA_GREEN, PURPLE},
  prelude::*,
  render::mesh::{CylinderAnchor, CylinderMeshBuilder},
};
use bevy_landmass::{
  debug::{EnableLandmassDebug, Landmass3dDebugPlugin},
  prelude::*,
};
use landmass_oxidized_navigation::{LandmassOxidizedNavigationPlugin, OxidizedArchipelago};
use oxidized_navigation::{
  NavMeshAffector, NavMeshSettings, OxidizedNavigationPlugin,
  debug_draw::{DrawNavMesh, OxidizedNavigationDebugDrawPlugin},
};

const AGENT_RADIUS: f32 = 0.35;
const START: Vec3 = Vec3::new(-7.0, 0.25, -3.0);
const GOAL_A: Vec3 = Vec3::new(7.0, 0.25, 3.0);
const GOAL_B: Vec3 = Vec3::new(-7.0, 0.25, -3.0);

fn main() {
  App::new()
    .add_plugins(DefaultPlugins.set(WindowPlugin {
      primary_window: Some(Window {
        title: "Avian + oxidized_navigation + landmass demo".to_owned(),
        ..default()
      }),
      ..default()
    }))
    .add_plugins(PhysicsPlugins::default())
    .add_plugins(OxidizedNavigationPlugin::<Collider>::new(NavMeshSettings {
      tile_width: 24,
      ..NavMeshSettings::from_agent_and_bounds(AGENT_RADIUS, 1.6, 20.0, -2.0)
    }))
    .add_plugins(OxidizedNavigationDebugDrawPlugin)
    .add_plugins(Landmass3dPlugin::default())
    .add_plugins(Landmass3dDebugPlugin::default())
    .add_plugins(LandmassOxidizedNavigationPlugin)
    .insert_resource(ClearColor(Color::srgb(0.05, 0.06, 0.07)))
    .add_systems(Startup, (setup, enable_debug_overlays))
    .add_systems(
      Update,
      (cycle_target, update_agent_velocity, move_agent_by_velocity).chain(),
    )
    .run();
}

#[derive(Component)]
struct DemoAgent;

#[derive(Component)]
struct Target;

#[derive(Resource)]
struct PatrolTarget {
  entity: Entity,
  points: [Vec3; 2],
  active_index: usize,
  timer: Timer,
}

fn setup(
  mut commands: Commands,
  mut meshes: ResMut<Assets<Mesh>>,
  mut materials: ResMut<Assets<StandardMaterial>>,
) {
  commands.spawn((
    Camera3d::default(),
    Transform::from_xyz(0.0, 11.0, 13.0).looking_at(Vec3::ZERO, Vec3::Y),
  ));

  commands.spawn((
    DirectionalLight {
      illuminance: 18_000.0,
      shadows_enabled: true,
      ..default()
    },
    Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -1.1, -0.6, 0.0)),
  ));

  spawn_floor(&mut commands, &mut meshes, &mut materials);
  spawn_obstacle(&mut commands, &mut meshes, &mut materials);

  let archipelago_entity = commands
    .spawn((
      Archipelago3d::new(AgentOptions::default_for_agent_radius(AGENT_RADIUS)),
      OxidizedArchipelago,
    ))
    .id();

  let target_entity = spawn_target(&mut commands, &mut meshes, &mut materials);
  spawn_agent(
    &mut commands,
    &mut meshes,
    &mut materials,
    archipelago_entity,
    target_entity,
  );

  commands.insert_resource(PatrolTarget {
    entity: target_entity,
    points: [GOAL_A, GOAL_B],
    active_index: 0,
    timer: Timer::from_seconds(7.0, TimerMode::Repeating),
  });
}

fn spawn_floor(
  commands: &mut Commands,
  meshes: &mut Assets<Mesh>,
  materials: &mut Assets<StandardMaterial>,
) {
  commands.spawn((
    Mesh3d(meshes.add(Plane3d::default().mesh().size(18.0, 12.0))),
    MeshMaterial3d(materials.add(Color::srgb(0.24, 0.30, 0.25))),
    Transform::IDENTITY,
    Collider::cuboid(18.0, 0.1, 12.0),
    NavMeshAffector,
  ));
}

fn spawn_obstacle(
  commands: &mut Commands,
  meshes: &mut Assets<Mesh>,
  materials: &mut Assets<StandardMaterial>,
) {
  commands.spawn((
    Mesh3d(meshes.add(Cuboid::new(1.2, 1.5, 5.0))),
    MeshMaterial3d(materials.add(StandardMaterial {
      base_color: DARK_GRAY.into(),
      perceptual_roughness: 0.9,
      ..default()
    })),
    Transform::from_xyz(0.0, 0.75, 0.0),
    Collider::cuboid(0.6, 0.75, 2.5),
    NavMeshAffector,
  ));
}

fn spawn_target(
  commands: &mut Commands,
  meshes: &mut Assets<Mesh>,
  materials: &mut Assets<StandardMaterial>,
) -> Entity {
  commands
    .spawn((
      Transform::from_translation(GOAL_A),
      Mesh3d(meshes.add(disc_mesh(0.45, 0.08))),
      MeshMaterial3d(materials.add(StandardMaterial {
        base_color: PURPLE.into(),
        emissive: PURPLE.into(),
        ..default()
      })),
      Target,
    ))
    .id()
}

fn spawn_agent(
  commands: &mut Commands,
  meshes: &mut Assets<Mesh>,
  materials: &mut Assets<StandardMaterial>,
  archipelago_entity: Entity,
  target_entity: Entity,
) {
  commands.spawn((
    Transform::from_translation(START),
    Mesh3d(meshes.add(disc_mesh(AGENT_RADIUS, 0.18))),
    MeshMaterial3d(materials.add(StandardMaterial {
      base_color: MEDIUM_SEA_GREEN.into(),
      emissive: MEDIUM_SEA_GREEN.into(),
      ..default()
    })),
    Agent3dBundle {
      agent: Default::default(),
      settings: AgentSettings {
        radius: AGENT_RADIUS,
        desired_speed: 2.0,
        max_speed: 3.0,
      },
      archipelago_ref: ArchipelagoRef3d::new(archipelago_entity),
    },
    AgentTarget3d::Entity(target_entity),
    DemoAgent,
  ));
}

fn disc_mesh(radius: f32, half_height: f32) -> Mesh {
  CylinderMeshBuilder {
    cylinder: Cylinder {
      radius,
      half_height,
    },
    resolution: 32,
    segments: 1,
    anchor: CylinderAnchor::MidPoint,
    caps: true,
  }
  .build()
}

fn enable_debug_overlays(
  mut nav_mesh_debug: ResMut<DrawNavMesh>,
  mut landmass_debug: ResMut<EnableLandmassDebug>,
) {
  nav_mesh_debug.0 = true;
  **landmass_debug = true;
}

fn cycle_target(
  time: Res<Time>,
  mut patrol: ResMut<PatrolTarget>,
  mut targets: Query<&mut Transform, With<Target>>,
) {
  patrol.timer.tick(time.delta());
  if !patrol.timer.just_finished() {
    return;
  }

  patrol.active_index = (patrol.active_index + 1) % patrol.points.len();
  if let Ok(mut transform) = targets.get_mut(patrol.entity) {
    transform.translation = patrol.points[patrol.active_index];
  }
}

fn update_agent_velocity(
  mut agents: Query<(&mut Velocity3d, &AgentDesiredVelocity3d), With<DemoAgent>>,
) {
  for (mut velocity, desired_velocity) in &mut agents {
    velocity.velocity = desired_velocity.velocity();
  }
}

fn move_agent_by_velocity(
  time: Res<Time>,
  mut agents: Query<(&mut Transform, &GlobalTransform, &Velocity3d), With<DemoAgent>>,
) {
  for (mut transform, global_transform, velocity) in &mut agents {
    let local_velocity = global_transform
      .affine()
      .inverse()
      .transform_vector3(velocity.velocity);
    transform.translation += local_velocity * time.delta_secs();
    transform.translation.y = START.y;

    if velocity.velocity.length_squared() > 0.001 {
      transform.look_to(velocity.velocity.normalize(), Vec3::Y);
    }
  }
}
