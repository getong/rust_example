mod net;
pub mod protocol;

use std::{
  collections::{HashMap, HashSet},
  time::Duration,
};

use avian3d::prelude::{Collider, Gravity, LinearVelocity, PhysicsPlugins, Position, RigidBody};
use bevy::{
  camera::ScalingMode, prelude::*, window::PresentMode, world_serialization::WorldInstanceReady,
};
use bevy_hanabi::prelude::*;

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
const FOX_GLTF_PATH: &str = "models/Fox.glb";
const CAMERA_VIEW_WIDTH: f32 = 1080.0;
const CAMERA_VIEW_HEIGHT: f32 = 820.0;
const CAMERA_FAR: f32 = 3000.0;
const PLAYER_MODEL_SCALE: f32 = 0.32;
const MONSTER_MODEL_SCALE: f32 = 0.28;
const ANIMATION_BLEND_MS: u64 = 160;

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

#[derive(Resource, Clone)]
struct SkeletalAnimationAssets {
  graph: Handle<AnimationGraph>,
  survey: AnimationNodeIndex,
  walk: AnimationNodeIndex,
  run: AnimationNodeIndex,
}

#[derive(Resource, Clone)]
struct VfxAssets {
  player_aura: Handle<EffectAsset>,
  monster_aura: Handle<EffectAsset>,
  burst: Handle<EffectAsset>,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct RemoteActor {
  id: u64,
}

#[derive(Component, Debug, Clone, Copy)]
struct ActorVisualState {
  last_position: Vec3,
  yaw: f32,
  last_vfx_pulse: u64,
}

impl ActorVisualState {
  fn new(actor: &ActorState) -> Self {
    Self {
      last_position: actor_ground_position(actor),
      yaw: 0.0,
      last_vfx_pulse: actor.vfx_pulse,
    }
  }
}

#[derive(Component, Debug, Clone, Copy)]
struct ActorAnimationBinding {
  actor_id: u64,
  initial_clip: ActorAnimationClip,
}

#[derive(Component, Debug, Clone, Copy)]
struct ActorAnimationPlayer {
  actor_id: u64,
}

#[derive(Component, Debug)]
struct TimedEffect(Timer);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActorAnimationClip {
  Survey,
  Walk,
  Run,
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
    .add_plugins(HanabiPlugin)
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
        sync_actor_animations.after(sync_actor_meshes),
        cleanup_timed_effects,
        update_status_text.after(net::drain_network_events),
      ),
    )
    .run();
}

fn setup_scene(
  mut commands: Commands,
  asset_server: Res<AssetServer>,
  mut meshes: ResMut<Assets<Mesh>>,
  mut materials: ResMut<Assets<StandardMaterial>>,
  mut animation_graphs: ResMut<Assets<AnimationGraph>>,
  mut effects: ResMut<Assets<EffectAsset>>,
) {
  let cube_mesh = meshes.add(Cuboid::default());
  let cylinder_mesh = meshes.add(Cylinder::new(1.0, 1.0));
  let animation_assets = create_skeletal_animation_assets(&asset_server, &mut animation_graphs);
  let vfx_assets = create_vfx_assets(&mut effects);

  commands.spawn((
    Camera3d::default(),
    Projection::from(OrthographicProjection {
      scaling_mode: ScalingMode::Fixed {
        width: CAMERA_VIEW_WIDTH,
        height: CAMERA_VIEW_HEIGHT,
      },
      far: CAMERA_FAR,
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
  commands.insert_resource(animation_assets);
  commands.insert_resource(vfx_assets);

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

fn create_skeletal_animation_assets(
  asset_server: &AssetServer,
  animation_graphs: &mut Assets<AnimationGraph>,
) -> SkeletalAnimationAssets {
  let (graph, nodes) = AnimationGraph::from_clips([
    asset_server.load(GltfAssetLabel::Animation(0).from_asset(FOX_GLTF_PATH)),
    asset_server.load(GltfAssetLabel::Animation(1).from_asset(FOX_GLTF_PATH)),
    asset_server.load(GltfAssetLabel::Animation(2).from_asset(FOX_GLTF_PATH)),
  ]);

  SkeletalAnimationAssets {
    graph: animation_graphs.add(graph),
    survey: nodes[0],
    walk: nodes[1],
    run: nodes[2],
  }
}

fn create_vfx_assets(effects: &mut Assets<EffectAsset>) -> VfxAssets {
  VfxAssets {
    player_aura: effects.add(create_actor_aura_effect(Vec4::new(0.2, 2.8, 5.0, 1.0))),
    monster_aura: effects.add(create_actor_aura_effect(Vec4::new(5.0, 0.6, 0.25, 1.0))),
    burst: effects.add(create_actor_burst_effect()),
  }
}

fn create_actor_aura_effect(color: Vec4) -> EffectAsset {
  let writer = ExprWriter::new();

  let init_age = SetAttributeModifier::new(Attribute::AGE, writer.lit(0.0).expr());
  let init_lifetime = SetAttributeModifier::new(Attribute::LIFETIME, writer.lit(0.85).expr());
  let init_position = SetPositionSphereModifier {
    center: writer.lit(Vec3::ZERO).expr(),
    radius: writer.lit(12.0).expr(),
    dimension: ShapeDimension::Surface,
  };
  let init_velocity = SetVelocitySphereModifier {
    center: writer.lit(Vec3::Y * 2.0).expr(),
    speed: writer.lit(4.0).expr(),
  };
  let update_accel = AccelModifier::new(writer.lit(Vec3::Y * 1.2).expr());

  let mut color_gradient = bevy_hanabi::Gradient::new();
  color_gradient.add_key(0.0, color);
  color_gradient.add_key(0.65, color * Vec4::new(0.55, 0.55, 0.55, 0.6));
  color_gradient.add_key(1.0, Vec4::ZERO);

  let mut size_gradient = bevy_hanabi::Gradient::new();
  size_gradient.add_key(0.0, Vec3::splat(2.5));
  size_gradient.add_key(0.7, Vec3::splat(5.5));
  size_gradient.add_key(1.0, Vec3::splat(0.1));

  EffectAsset::new(2048, SpawnerSettings::rate(90.0.into()), writer.finish())
    .with_name("actor_aura")
    .init(init_position)
    .init(init_velocity)
    .init(init_age)
    .init(init_lifetime)
    .update(update_accel)
    .render(ColorOverLifetimeModifier::new(color_gradient))
    .render(SizeOverLifetimeModifier {
      gradient: size_gradient,
      screen_space_size: false,
    })
}

fn create_actor_burst_effect() -> EffectAsset {
  let writer = ExprWriter::new();

  let init_age = SetAttributeModifier::new(Attribute::AGE, writer.lit(0.0).expr());
  let init_lifetime = SetAttributeModifier::new(Attribute::LIFETIME, writer.lit(0.55).expr());
  let init_position = SetPositionSphereModifier {
    center: writer.lit(Vec3::ZERO).expr(),
    radius: writer.lit(6.0).expr(),
    dimension: ShapeDimension::Volume,
  };
  let init_velocity = SetVelocitySphereModifier {
    center: writer.lit(Vec3::ZERO).expr(),
    speed: writer.lit(55.0).expr(),
  };
  let update_drag = LinearDragModifier::new(writer.lit(4.5).expr());
  let update_accel = AccelModifier::new(writer.lit(Vec3::Y * -3.0).expr());

  let mut color_gradient = bevy_hanabi::Gradient::new();
  color_gradient.add_key(0.0, Vec4::new(5.0, 4.0, 1.2, 1.0));
  color_gradient.add_key(0.45, Vec4::new(2.5, 0.7, 0.3, 0.7));
  color_gradient.add_key(1.0, Vec4::ZERO);

  let mut size_gradient = bevy_hanabi::Gradient::new();
  size_gradient.add_key(0.0, Vec3::splat(8.0));
  size_gradient.add_key(0.45, Vec3::splat(3.5));
  size_gradient.add_key(1.0, Vec3::splat(0.0));

  EffectAsset::new(512, SpawnerSettings::once(96.0.into()), writer.finish())
    .with_name("actor_burst")
    .init(init_position)
    .init(init_velocity)
    .init(init_age)
    .init(init_lifetime)
    .update(update_drag)
    .update(update_accel)
    .render(ColorOverLifetimeModifier::new(color_gradient))
    .render(SizeOverLifetimeModifier {
      gradient: size_gradient,
      screen_space_size: false,
    })
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
  asset_server: Res<AssetServer>,
  vfx_assets: Res<VfxAssets>,
  mut actors: Query<(
    Entity,
    &RemoteActor,
    &mut Transform,
    Option<&mut Position>,
    &mut ActorVisualState,
  )>,
) {
  let snapshot_ids: HashSet<u64> = world.actors.keys().copied().collect();
  let mut rendered_ids = HashSet::new();

  for (entity, remote_actor, mut transform, physics_position, mut visual_state) in &mut actors {
    let Some(actor) = world.actors.get(&remote_actor.id) else {
      commands.entity(entity).despawn();
      continue;
    };

    rendered_ids.insert(remote_actor.id);
    apply_actor_visual(
      actor,
      &mut transform,
      physics_position,
      &mut visual_state,
      &mut commands,
      &vfx_assets,
    );
  }

  for actor in world.actors.values() {
    if rendered_ids.contains(&actor.id) || !snapshot_ids.contains(&actor.id) {
      continue;
    }

    spawn_animated_actor(&mut commands, &asset_server, &vfx_assets, actor);
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

fn spawn_animated_actor(
  commands: &mut Commands,
  asset_server: &AssetServer,
  vfx_assets: &VfxAssets,
  actor: &ActorState,
) {
  let actor_kind = actor_kind(actor);
  commands
    .spawn((
      actor_transform(actor, 0.0),
      Position::new(actor_position(actor)),
      RigidBody::Kinematic,
      Collider::cuboid(
        actor_size(actor).x,
        actor_size(actor).y,
        actor_size(actor).z,
      ),
      LinearVelocity::ZERO,
      RemoteActor { id: actor.id },
      ActorVisualState::new(actor),
      Visibility::default(),
      Name::new("Animated Remote Actor"),
    ))
    .with_children(|parent| {
      parent
        .spawn((
          WorldAssetRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(FOX_GLTF_PATH))),
          actor_model_transform(actor),
          ActorAnimationBinding {
            actor_id: actor.id,
            initial_clip: desired_animation_clip(actor),
          },
          Name::new("Actor Skeleton"),
        ))
        .observe(configure_actor_animation_when_ready);

      parent.spawn((
        ParticleEffect::new(actor_aura_effect(vfx_assets, actor_kind)),
        Transform::from_translation(Vec3::Y * actor_size(actor).y * 0.5),
        Name::new("Actor Aura"),
      ));
    });

  spawn_actor_burst(commands, vfx_assets, actor_effect_position(actor));
}

fn configure_actor_animation_when_ready(
  scene_ready: On<WorldInstanceReady>,
  mut commands: Commands,
  animation_assets: Res<SkeletalAnimationAssets>,
  children: Query<&Children>,
  bindings: Query<&ActorAnimationBinding>,
  mut players: Query<&mut AnimationPlayer>,
) {
  let Ok(binding) = bindings.get(scene_ready.entity) else {
    return;
  };

  for child in children.iter_descendants(scene_ready.entity) {
    let Ok(mut player) = players.get_mut(child) else {
      continue;
    };

    let mut transitions = AnimationTransitions::new();
    transitions
      .play(
        &mut player,
        animation_node(&animation_assets, binding.initial_clip),
        Duration::ZERO,
      )
      .repeat();

    commands
      .entity(child)
      .insert(AnimationGraphHandle(animation_assets.graph.clone()))
      .insert(transitions)
      .insert(ActorAnimationPlayer {
        actor_id: binding.actor_id,
      });
  }
}

fn sync_actor_animations(
  world: Res<ClientWorld>,
  animation_assets: Res<SkeletalAnimationAssets>,
  mut players: Query<(
    &ActorAnimationPlayer,
    &mut AnimationPlayer,
    &mut AnimationTransitions,
  )>,
) {
  for (binding, mut player, mut transitions) in &mut players {
    let Some(actor) = world.actors.get(&binding.actor_id) else {
      continue;
    };

    let desired_clip = desired_animation_clip(actor);
    let desired_node = animation_node(&animation_assets, desired_clip);
    let current_node = player
      .playing_animations()
      .next()
      .map(|(node_index, _)| *node_index);

    if current_node != Some(desired_node) {
      transitions
        .play(
          &mut player,
          desired_node,
          Duration::from_millis(ANIMATION_BLEND_MS),
        )
        .repeat();
    }

    if let Some(active_animation) = player.animation_mut(desired_node) {
      active_animation.set_speed(animation_speed(actor, desired_clip));
    }
  }
}

fn actor_aura_effect(vfx_assets: &VfxAssets, actor_kind: ActorKind) -> Handle<EffectAsset> {
  match actor_kind {
    ActorKind::Player => vfx_assets.player_aura.clone(),
    ActorKind::Monster | ActorKind::Unknown => vfx_assets.monster_aura.clone(),
  }
}

fn spawn_actor_burst(commands: &mut Commands, vfx_assets: &VfxAssets, position: Vec3) {
  commands.spawn((
    ParticleEffect::new(vfx_assets.burst.clone()),
    Transform::from_translation(position),
    TimedEffect(Timer::from_seconds(1.2, TimerMode::Once)),
    Name::new("Actor Burst VFX"),
  ));
}

fn cleanup_timed_effects(
  mut commands: Commands,
  time: Res<Time>,
  mut effects: Query<(Entity, &mut TimedEffect)>,
) {
  for (entity, mut effect) in &mut effects {
    effect.0.tick(time.delta());
    if effect.0.just_finished() {
      commands.entity(entity).despawn();
    }
  }
}

fn apply_actor_visual(
  actor: &ActorState,
  transform: &mut Transform,
  physics_position: Option<Mut<Position>>,
  visual_state: &mut ActorVisualState,
  commands: &mut Commands,
  vfx_assets: &VfxAssets,
) {
  let position = actor_ground_position(actor);
  let movement = position - visual_state.last_position;
  if movement.xz().length_squared() > 1.0 {
    visual_state.yaw = movement.x.atan2(movement.z);
  }
  visual_state.last_position = position;

  *transform = actor_transform(actor, visual_state.yaw);
  if let Some(mut physics_position) = physics_position {
    physics_position.0 = actor_position(actor);
  }

  if actor.vfx_pulse != visual_state.last_vfx_pulse {
    visual_state.last_vfx_pulse = actor.vfx_pulse;
    spawn_actor_burst(commands, vfx_assets, actor_effect_position(actor));
  }
}

fn actor_kind(actor: &ActorState) -> ActorKind {
  ActorKind::try_from(actor.kind).unwrap_or(ActorKind::Unknown)
}

fn actor_position(actor: &ActorState) -> Vec3 {
  Vec3::new(actor.x, actor.y, actor.z)
}

fn actor_ground_position(actor: &ActorState) -> Vec3 {
  Vec3::new(actor.x, 0.0, actor.z)
}

fn actor_effect_position(actor: &ActorState) -> Vec3 {
  Vec3::new(actor.x, actor_size(actor).y * 0.5, actor.z)
}

fn actor_transform(actor: &ActorState, yaw: f32) -> Transform {
  Transform::from_translation(actor_ground_position(actor))
    .with_rotation(Quat::from_rotation_y(yaw))
}

fn actor_model_transform(actor: &ActorState) -> Transform {
  Transform::from_scale(Vec3::splat(actor_model_scale(actor)))
}

fn actor_model_scale(actor: &ActorState) -> f32 {
  match actor_kind(actor) {
    ActorKind::Player => PLAYER_MODEL_SCALE,
    ActorKind::Monster => MONSTER_MODEL_SCALE,
    ActorKind::Unknown => PLAYER_MODEL_SCALE * 0.75,
  }
}

fn actor_size(actor: &ActorState) -> Vec3 {
  match actor_kind(actor) {
    ActorKind::Player => PLAYER_SIZE,
    ActorKind::Monster => MONSTER_SIZE,
    ActorKind::Unknown => Vec3::splat(18.0),
  }
}

fn desired_animation_clip(actor: &ActorState) -> ActorAnimationClip {
  if actor.blue <= 0 || actor.motion_speed <= 5.0 {
    ActorAnimationClip::Survey
  } else if actor.motion_speed >= 160.0 {
    ActorAnimationClip::Run
  } else {
    ActorAnimationClip::Walk
  }
}

fn animation_node(
  animation_assets: &SkeletalAnimationAssets,
  clip: ActorAnimationClip,
) -> AnimationNodeIndex {
  match clip {
    ActorAnimationClip::Survey => animation_assets.survey,
    ActorAnimationClip::Walk => animation_assets.walk,
    ActorAnimationClip::Run => animation_assets.run,
  }
}

fn animation_speed(actor: &ActorState, clip: ActorAnimationClip) -> f32 {
  match clip {
    ActorAnimationClip::Survey => 1.0,
    ActorAnimationClip::Walk => (actor.motion_speed / 90.0).clamp(0.6, 1.4),
    ActorAnimationClip::Run => (actor.motion_speed / 180.0).clamp(0.85, 2.2),
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
