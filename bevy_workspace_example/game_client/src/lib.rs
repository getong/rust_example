mod net;
pub mod protocol;

use std::collections::{HashMap, HashSet};

use bevy::{prelude::*, window::PresentMode};

use crate::{
  net::NetworkClient,
  protocol::{
    ActorKind, ActorState, ClientEnvelope, MapState, ObstacleShape, ObstacleState, PlayerInput,
    client_envelope,
  },
};

const ARENA_SIZE: Vec2 = Vec2::new(840.0, 600.0);
const PLAYER_SIZE: Vec2 = Vec2::splat(28.0);
const MONSTER_SIZE: Vec2 = Vec2::splat(24.0);
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
        title: "Game Client - WASD to move".into(),
        resolution: (960, 720).into(),
        present_mode: PresentMode::AutoVsync,
        ..default()
      }),
      ..default()
    }))
    .init_resource::<ClientWorld>()
    .init_resource::<InputSendClock>()
    .add_systems(Startup, (setup_scene, net::start_network_client))
    .add_systems(
      Update,
      (
        net::drain_network_events,
        send_player_input.after(net::drain_network_events),
        sync_obstacle_sprites.after(net::drain_network_events),
        sync_actor_sprites.after(net::drain_network_events),
        update_status_text.after(net::drain_network_events),
      ),
    )
    .run();
}

fn setup_scene(mut commands: Commands) {
  commands.spawn(Camera2d);
  commands.spawn((
    Sprite::from_color(Color::srgb(0.09, 0.10, 0.11), ARENA_SIZE),
    Transform::from_xyz(0.0, 0.0, -2.0),
  ));
  commands.spawn((
    Sprite::from_color(
      Color::srgb(0.18, 0.19, 0.20),
      ARENA_SIZE + Vec2::splat(10.0),
    ),
    Transform::from_xyz(0.0, 0.0, -3.0),
  ));
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
    })),
  };
  let _ = network.sender.send(envelope);
}

fn sync_obstacle_sprites(
  mut commands: Commands,
  world: Res<ClientWorld>,
  mut obstacles: Query<(Entity, &RemoteObstacle, &mut Transform, &mut Sprite)>,
) {
  let Some(map) = &world.map else {
    return;
  };

  let mut rendered_indices = HashSet::new();
  for (entity, remote_obstacle, mut transform, mut sprite) in &mut obstacles {
    let Some(obstacle) = map.obstacles.get(remote_obstacle.index) else {
      commands.entity(entity).despawn();
      continue;
    };

    rendered_indices.insert(remote_obstacle.index);
    apply_obstacle_visual(obstacle, &mut transform, &mut sprite);
  }

  for (index, obstacle) in map.obstacles.iter().enumerate() {
    if rendered_indices.contains(&index) {
      continue;
    }

    let mut transform = Transform::from_xyz(obstacle.x, obstacle.y, -1.0);
    let mut sprite = Sprite::from_color(obstacle_color(obstacle), obstacle_size(obstacle));
    apply_obstacle_visual(obstacle, &mut transform, &mut sprite);
    commands.spawn((sprite, transform, RemoteObstacle { index }));
  }
}

fn sync_actor_sprites(
  mut commands: Commands,
  world: Res<ClientWorld>,
  mut actors: Query<(Entity, &RemoteActor, &mut Transform, &mut Sprite)>,
) {
  let snapshot_ids: HashSet<u64> = world.actors.keys().copied().collect();
  let mut rendered_ids = HashSet::new();

  for (entity, remote_actor, mut transform, mut sprite) in &mut actors {
    let Some(actor) = world.actors.get(&remote_actor.id) else {
      commands.entity(entity).despawn();
      continue;
    };

    rendered_ids.insert(remote_actor.id);
    transform.translation = Vec3::new(actor.x, actor.y, actor_z(actor));
    sprite.color = actor_color(actor, world.local_actor_id);
  }

  for actor in world.actors.values() {
    if rendered_ids.contains(&actor.id) || !snapshot_ids.contains(&actor.id) {
      continue;
    }

    commands.spawn((
      Sprite::from_color(actor_color(actor, world.local_actor_id), actor_size(actor)),
      Transform::from_xyz(actor.x, actor.y, actor_z(actor)),
      RemoteActor { id: actor.id },
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

fn input_direction(keyboard: &ButtonInput<KeyCode>) -> Vec2 {
  let mut direction = Vec2::ZERO;
  if keyboard.pressed(KeyCode::KeyW) || keyboard.pressed(KeyCode::ArrowUp) {
    direction.y += 1.0;
  }
  if keyboard.pressed(KeyCode::KeyS) || keyboard.pressed(KeyCode::ArrowDown) {
    direction.y -= 1.0;
  }
  if keyboard.pressed(KeyCode::KeyA) || keyboard.pressed(KeyCode::ArrowLeft) {
    direction.x -= 1.0;
  }
  if keyboard.pressed(KeyCode::KeyD) || keyboard.pressed(KeyCode::ArrowRight) {
    direction.x += 1.0;
  }
  direction.normalize_or_zero()
}

fn actor_kind(actor: &ActorState) -> ActorKind {
  ActorKind::try_from(actor.kind).unwrap_or(ActorKind::Unknown)
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

fn actor_size(actor: &ActorState) -> Vec2 {
  match actor_kind(actor) {
    ActorKind::Player => PLAYER_SIZE,
    ActorKind::Monster => MONSTER_SIZE,
    ActorKind::Unknown => Vec2::splat(18.0),
  }
}

fn actor_z(actor: &ActorState) -> f32 {
  match actor_kind(actor) {
    ActorKind::Player => 2.0,
    ActorKind::Monster => 1.0,
    ActorKind::Unknown => 0.0,
  }
}

fn apply_obstacle_visual(obstacle: &ObstacleState, transform: &mut Transform, sprite: &mut Sprite) {
  transform.translation = Vec3::new(obstacle.x, obstacle.y, -1.0);
  transform.rotation = match obstacle_shape(obstacle) {
    ObstacleShape::Diamond => Quat::from_rotation_z(std::f32::consts::FRAC_PI_4),
    _ => Quat::IDENTITY,
  };
  sprite.custom_size = Some(obstacle_size(obstacle));
  sprite.color = obstacle_color(obstacle);
}

fn obstacle_shape(obstacle: &ObstacleState) -> ObstacleShape {
  ObstacleShape::try_from(obstacle.shape).unwrap_or(ObstacleShape::Rectangle)
}

fn obstacle_size(obstacle: &ObstacleState) -> Vec2 {
  Vec2::new(obstacle.width, obstacle.height)
}

fn obstacle_color(obstacle: &ObstacleState) -> Color {
  match obstacle_shape(obstacle) {
    ObstacleShape::Rectangle => Color::srgb(0.42, 0.44, 0.48),
    ObstacleShape::Diamond => Color::srgb(0.42, 0.34, 0.62),
    ObstacleShape::Ellipse => Color::srgb(0.30, 0.52, 0.58),
    ObstacleShape::Cross => Color::srgb(0.58, 0.40, 0.28),
  }
}
