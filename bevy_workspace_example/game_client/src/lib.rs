mod net;
pub mod protocol;

use std::{
  collections::{HashMap, HashSet},
  time::Duration,
};

use avian3d::prelude::{Collider, Gravity, LinearVelocity, PhysicsPlugins, Position, RigidBody};
use bevy::{camera::ScalingMode, prelude::*, window::PresentMode};

use crate::{
  net::NetworkClient,
  protocol::{
    ActorKind, ActorState, ClientEnvelope, MapState, ObstacleShape, ObstacleState, PlayerInput,
    client_envelope,
  },
};

const ARENA_HALF_WIDTH: f32 = 420.0;
const ARENA_HALF_DEPTH: f32 = 300.0;
const FLOOR_THICKNESS: f32 = 2.0;
const WALL_THICKNESS: f32 = 20.0;
const WALL_HEIGHT: f32 = 48.0;
const PLAYER_SIZE: Vec3 = Vec3::new(28.0, 36.0, 28.0);
const MONSTER_SIZE: Vec3 = Vec3::new(24.0, 32.0, 24.0);
const INPUT_SEND_SECONDS: f32 = 1.0 / 30.0;

#[derive(Resource, Debug)]
pub(crate) struct ClientWorld {
  local_actor_id: Option<u64>,
  tick: u64,
  actors: HashMap<u64, ActorState>,
  map: Option<MapState>,
  status: String,
}

impl Default for ClientWorld {
  fn default() -> Self {
    Self {
      local_actor_id: None,
      tick: 0,
      actors: HashMap::new(),
      map: None,
      status: "connecting to 127.0.0.1:6000".to_string(),
    }
  }
}

#[derive(Resource, Debug)]
struct InputSendClock(Timer);

impl Default for InputSendClock {
  fn default() -> Self {
    Self(Timer::from_seconds(
      INPUT_SEND_SECONDS,
      TimerMode::Repeating,
    ))
  }
}

#[derive(Resource, Clone)]
struct SceneAssets {
  cube_mesh: Handle<Mesh>,
  cylinder_mesh: Handle<Mesh>,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct RemoteActor {
  id: u64,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct RemoteObstacle {
  index: usize,
}

#[derive(Component)]
struct StatusText;

pub fn run() {
  App::new()
    .add_plugins(DefaultPlugins.set(WindowPlugin {
      primary_window: Some(Window {
        title: "Game Client 3D - WASD to move".into(),
        resolution: (960, 720).into(),
        present_mode: PresentMode::AutoVsync,
        ..default()
      }),
      ..default()
    }))
    .add_plugins(PhysicsPlugins::default())
    .insert_resource(Gravity::ZERO)
    .add_plugins(lightyear::prelude::client::ClientPlugins {
      tick_duration: Duration::from_secs_f64(INPUT_SEND_SECONDS as f64),
    })
    .add_plugins(protocol::GameProtocolPlugin)
    .init_resource::<ClientWorld>()
    .init_resource::<InputSendClock>()
    .add_systems(Startup, (setup_scene, net::start_network_client))
    .add_systems(
      Update,
      (
        net::drain_network_events,
        send_player_input.after(net::drain_network_events),
        sync_obstacle_meshes.after(net::drain_network_events),
        sync_actor_meshes.after(net::drain_network_events),
        update_status_text.after(net::drain_network_events),
      ),
    )
    .run();
}

fn setup_scene(
  mut commands: Commands,
  mut meshes: ResMut<Assets<Mesh>>,
  mut materials: ResMut<Assets<StandardMaterial>>,
) {
  let cube_mesh = meshes.add(Cuboid::default());
  let cylinder_mesh = meshes.add(Cylinder::new(1.0, 1.0));

  commands.spawn((
    Camera3d::default(),
    Projection::from(OrthographicProjection {
      scaling_mode: ScalingMode::Fixed {
        width: 960.0,
        height: 720.0,
      },
      ..OrthographicProjection::default_3d()
    }),
    Transform::from_xyz(0.0, 560.0, 520.0).looking_at(Vec3::ZERO, Vec3::Y),
    IsDefaultUiCamera,
    AmbientLight {
      brightness: 120.0,
      ..default()
    },
  ));

  commands.spawn((
    DirectionalLight {
      illuminance: 18_000.0,
      shadow_maps_enabled: true,
      ..default()
    },
    Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -1.05, -0.45, 0.0)),
  ));

  spawn_static_scene(&mut commands, &cube_mesh, &mut materials);
  commands.insert_resource(SceneAssets {
    cube_mesh,
    cylinder_mesh,
  });

  commands.spawn((
    Text::new("connecting..."),
    TextFont {
      font_size: FontSize::Px(18.0),
      ..default()
    },
    TextColor(Color::srgb(0.92, 0.94, 0.96)),
    TextShadow::default(),
    Node {
      position_type: PositionType::Absolute,
      top: px(14.0),
      left: px(16.0),
      ..default()
    },
    StatusText,
  ));
}

fn spawn_static_scene(
  commands: &mut Commands,
  cube_mesh: &Handle<Mesh>,
  materials: &mut Assets<StandardMaterial>,
) {
  spawn_static_cuboid(
    commands,
    cube_mesh,
    materials,
    Vec3::new(0.0, -FLOOR_THICKNESS * 0.5, 0.0),
    Vec3::new(
      ARENA_HALF_WIDTH * 2.0,
      FLOOR_THICKNESS,
      ARENA_HALF_DEPTH * 2.0,
    ),
    Color::srgb(0.11, 0.14, 0.12),
    "Arena Floor",
  );

  let wall_y = WALL_HEIGHT * 0.5;
  let horizontal_size = Vec3::new(
    ARENA_HALF_WIDTH * 2.0 + WALL_THICKNESS * 2.0,
    WALL_HEIGHT,
    WALL_THICKNESS,
  );
  let vertical_size = Vec3::new(WALL_THICKNESS, WALL_HEIGHT, ARENA_HALF_DEPTH * 2.0);

  for (center, size) in [
    (
      Vec3::new(0.0, wall_y, ARENA_HALF_DEPTH + WALL_THICKNESS * 0.5),
      horizontal_size,
    ),
    (
      Vec3::new(0.0, wall_y, -ARENA_HALF_DEPTH - WALL_THICKNESS * 0.5),
      horizontal_size,
    ),
    (
      Vec3::new(ARENA_HALF_WIDTH + WALL_THICKNESS * 0.5, wall_y, 0.0),
      vertical_size,
    ),
    (
      Vec3::new(-ARENA_HALF_WIDTH - WALL_THICKNESS * 0.5, wall_y, 0.0),
      vertical_size,
    ),
  ] {
    spawn_static_cuboid(
      commands,
      cube_mesh,
      materials,
      center,
      size,
      Color::srgb(0.24, 0.25, 0.27),
      "Arena Wall",
    );
  }
}

fn spawn_static_cuboid(
  commands: &mut Commands,
  cube_mesh: &Handle<Mesh>,
  materials: &mut Assets<StandardMaterial>,
  center: Vec3,
  size: Vec3,
  color: Color,
  name: &'static str,
) {
  commands.spawn((
    Mesh3d(cube_mesh.clone()),
    MeshMaterial3d(materials.add(StandardMaterial {
      base_color: color,
      perceptual_roughness: 0.9,
      ..default()
    })),
    Transform::from_translation(center).with_scale(size),
    Position::new(center),
    RigidBody::Static,
    Collider::cuboid(1.0, 1.0, 1.0),
    Name::new(name),
  ));
}

fn send_player_input(
  keyboard: Res<ButtonInput<KeyCode>>,
  time: Res<Time>,
  mut clock: ResMut<InputSendClock>,
  network: Option<ResMut<NetworkClient>>,
) {
  let Some(mut network) = network else {
    return;
  };
  if !network.connected {
    return;
  }

  clock.0.tick(time.delta());
  if !clock.0.just_finished() {
    return;
  }

  let direction = input_direction(&keyboard);
  network.sequence += 1;
  let envelope = ClientEnvelope {
    payload: Some(client_envelope::Payload::Input(PlayerInput {
      sequence: network.sequence,
      x: direction.x,
      y: direction.y,
      z: direction.z,
    })),
  };
  let _ = network.sender.send(envelope);
}

fn sync_obstacle_meshes(
  mut commands: Commands,
  world: Res<ClientWorld>,
  scene_assets: Res<SceneAssets>,
  mut meshes: Query<(
    Entity,
    &RemoteObstacle,
    &mut Transform,
    &mut Mesh3d,
    &MeshMaterial3d<StandardMaterial>,
    Option<&mut Position>,
  )>,
  mut materials: ResMut<Assets<StandardMaterial>>,
) {
  let Some(map) = &world.map else {
    return;
  };

  let mut rendered_indices = HashSet::new();
  for (entity, remote_obstacle, mut transform, mut mesh, material, physics_position) in &mut meshes
  {
    let Some(obstacle) = map.obstacles.get(remote_obstacle.index) else {
      commands.entity(entity).despawn();
      continue;
    };

    rendered_indices.insert(remote_obstacle.index);
    apply_obstacle_visual(
      obstacle,
      &scene_assets,
      &mut transform,
      &mut mesh,
      material,
      physics_position,
      &mut materials,
    );
  }

  for (index, obstacle) in map.obstacles.iter().enumerate() {
    if rendered_indices.contains(&index) {
      continue;
    }

    let center = obstacle_position(obstacle);
    commands.spawn((
      Mesh3d(obstacle_mesh(obstacle, &scene_assets)),
      MeshMaterial3d(materials.add(StandardMaterial {
        base_color: obstacle_color(obstacle),
        perceptual_roughness: 0.88,
        ..default()
      })),
      obstacle_transform(obstacle),
      Position::new(center),
      RigidBody::Static,
      obstacle_collider(obstacle),
      RemoteObstacle { index },
      Name::new("Remote Obstacle"),
    ));
  }
}

fn sync_actor_meshes(
  mut commands: Commands,
  world: Res<ClientWorld>,
  scene_assets: Res<SceneAssets>,
  mut actors: Query<(
    Entity,
    &RemoteActor,
    &mut Transform,
    &MeshMaterial3d<StandardMaterial>,
    Option<&mut Position>,
  )>,
  mut materials: ResMut<Assets<StandardMaterial>>,
) {
  let snapshot_ids: HashSet<u64> = world.actors.keys().copied().collect();
  let mut rendered_ids = HashSet::new();

  for (entity, remote_actor, mut transform, material, physics_position) in &mut actors {
    let Some(actor) = world.actors.get(&remote_actor.id) else {
      commands.entity(entity).despawn();
      continue;
    };

    rendered_ids.insert(remote_actor.id);
    apply_actor_visual(
      actor,
      world.local_actor_id,
      &mut transform,
      material,
      physics_position,
      &mut materials,
    );
  }

  for actor in world.actors.values() {
    if rendered_ids.contains(&actor.id) || !snapshot_ids.contains(&actor.id) {
      continue;
    }

    commands.spawn((
      Mesh3d(scene_assets.cube_mesh.clone()),
      MeshMaterial3d(materials.add(StandardMaterial {
        base_color: actor_color(actor, world.local_actor_id),
        perceptual_roughness: 0.72,
        ..default()
      })),
      actor_transform(actor),
      Position::new(actor_position(actor)),
      RigidBody::Kinematic,
      Collider::cuboid(1.0, 1.0, 1.0),
      LinearVelocity::ZERO,
      RemoteActor { id: actor.id },
      Name::new("Remote Actor"),
    ));
  }
}

fn update_status_text(world: Res<ClientWorld>, mut text_query: Query<&mut Text, With<StatusText>>) {
  for mut text in &mut text_query {
    let player = world
      .local_actor_id
      .map_or_else(|| "-".to_string(), |id| id.to_string());
    let map = world
      .map
      .as_ref()
      .map_or("loading", |map| map.name.as_str());
    let obstacles = world.map.as_ref().map_or(0, |map| map.obstacles.len());
    text.0 = format!(
      "{} | map {} | tick {} | local actor {} | actors {} | obstacles {}",
      world.status,
      map,
      world.tick,
      player,
      world.actors.len(),
      obstacles
    );
  }
}

fn input_direction(keyboard: &ButtonInput<KeyCode>) -> Vec3 {
  let mut direction = Vec3::ZERO;
  if keyboard.pressed(KeyCode::KeyW) || keyboard.pressed(KeyCode::ArrowUp) {
    direction.z -= 1.0;
  }
  if keyboard.pressed(KeyCode::KeyS) || keyboard.pressed(KeyCode::ArrowDown) {
    direction.z += 1.0;
  }
  if keyboard.pressed(KeyCode::KeyA) || keyboard.pressed(KeyCode::ArrowLeft) {
    direction.x -= 1.0;
  }
  if keyboard.pressed(KeyCode::KeyD) || keyboard.pressed(KeyCode::ArrowRight) {
    direction.x += 1.0;
  }
  direction.normalize_or_zero()
}

fn apply_actor_visual(
  actor: &ActorState,
  local_actor_id: Option<u64>,
  transform: &mut Transform,
  material: &MeshMaterial3d<StandardMaterial>,
  physics_position: Option<Mut<Position>>,
  materials: &mut Assets<StandardMaterial>,
) {
  *transform = actor_transform(actor);
  if let Some(mut physics_position) = physics_position {
    physics_position.0 = actor_position(actor);
  }
  if let Some(mut material) = materials.get_mut(&material.0) {
    material.base_color = actor_color(actor, local_actor_id);
  }
}

fn actor_kind(actor: &ActorState) -> ActorKind {
  ActorKind::try_from(actor.kind).unwrap_or(ActorKind::Unknown)
}

fn actor_position(actor: &ActorState) -> Vec3 {
  Vec3::new(actor.x, actor.y, actor.z)
}

fn actor_transform(actor: &ActorState) -> Transform {
  Transform::from_translation(actor_position(actor)).with_scale(actor_size(actor))
}

fn actor_color(actor: &ActorState, local_actor_id: Option<u64>) -> Color {
  if Some(actor.id) == local_actor_id {
    return Color::srgb(0.20, 0.70, 1.00);
  }

  match actor_kind(actor) {
    ActorKind::Player => Color::srgb(0.28, 0.86, 0.42),
    ActorKind::Monster => {
      if actor.blue <= 0 {
        Color::srgb(0.28, 0.28, 0.30)
      } else {
        Color::srgb(0.90, 0.26, 0.22)
      }
    }
    ActorKind::Unknown => Color::srgb(0.75, 0.75, 0.75),
  }
}

fn actor_size(actor: &ActorState) -> Vec3 {
  match actor_kind(actor) {
    ActorKind::Player => PLAYER_SIZE,
    ActorKind::Monster => MONSTER_SIZE,
    ActorKind::Unknown => Vec3::splat(18.0),
  }
}

fn apply_obstacle_visual(
  obstacle: &ObstacleState,
  scene_assets: &SceneAssets,
  transform: &mut Transform,
  mesh: &mut Mesh3d,
  material: &MeshMaterial3d<StandardMaterial>,
  physics_position: Option<Mut<Position>>,
  materials: &mut Assets<StandardMaterial>,
) {
  *transform = obstacle_transform(obstacle);
  mesh.0 = obstacle_mesh(obstacle, scene_assets);
  if let Some(mut physics_position) = physics_position {
    physics_position.0 = obstacle_position(obstacle);
  }
  if let Some(mut material) = materials.get_mut(&material.0) {
    material.base_color = obstacle_color(obstacle);
  }
}

fn obstacle_shape(obstacle: &ObstacleState) -> ObstacleShape {
  ObstacleShape::try_from(obstacle.shape).unwrap_or(ObstacleShape::Cuboid)
}

fn obstacle_position(obstacle: &ObstacleState) -> Vec3 {
  Vec3::new(obstacle.x, obstacle.y, obstacle.z)
}

fn obstacle_size(obstacle: &ObstacleState) -> Vec3 {
  Vec3::new(obstacle.width, obstacle.height, obstacle.depth)
}

fn obstacle_transform(obstacle: &ObstacleState) -> Transform {
  let size = obstacle_size(obstacle);
  let mut transform = Transform::from_translation(obstacle_position(obstacle));
  transform.scale = match obstacle_shape(obstacle) {
    ObstacleShape::Cylinder => Vec3::new(size.x * 0.5, size.y, size.z * 0.5),
    _ => size,
  };
  transform.rotation = match obstacle_shape(obstacle) {
    ObstacleShape::DiamondPrism => Quat::from_rotation_y(std::f32::consts::FRAC_PI_4),
    _ => Quat::IDENTITY,
  };
  transform
}

fn obstacle_mesh(obstacle: &ObstacleState, scene_assets: &SceneAssets) -> Handle<Mesh> {
  match obstacle_shape(obstacle) {
    ObstacleShape::Cylinder => scene_assets.cylinder_mesh.clone(),
    ObstacleShape::Cuboid | ObstacleShape::DiamondPrism | ObstacleShape::Cross => {
      scene_assets.cube_mesh.clone()
    }
  }
}

fn obstacle_collider(obstacle: &ObstacleState) -> Collider {
  match obstacle_shape(obstacle) {
    ObstacleShape::Cylinder => Collider::cylinder(1.0, 1.0),
    ObstacleShape::Cuboid | ObstacleShape::DiamondPrism | ObstacleShape::Cross => {
      Collider::cuboid(1.0, 1.0, 1.0)
    }
  }
}

fn obstacle_color(obstacle: &ObstacleState) -> Color {
  match obstacle_shape(obstacle) {
    ObstacleShape::Cuboid => Color::srgb(0.42, 0.44, 0.48),
    ObstacleShape::DiamondPrism => Color::srgb(0.42, 0.34, 0.62),
    ObstacleShape::Cylinder => Color::srgb(0.30, 0.52, 0.58),
    ObstacleShape::Cross => Color::srgb(0.58, 0.40, 0.28),
  }
}
