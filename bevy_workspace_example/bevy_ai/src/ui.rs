use bevy::{picking::hover::Hovered, prelude::*, ui::Pressed, ui_widgets::Button};
use bevy_state::prelude::{DespawnOnExit, NextState, State};

use crate::{
  actors::{ActorKind, ArenaPosition, MaxHealth, Monster, Player, RedBlueValues},
  levels::{GameLevel, LevelEntity, all_levels, level_number, level_total},
  player_state::PlayerDefeated,
  terrain::{LevelMap, game_to_world_position},
};

const HEAL_AMOUNT: i32 = 140;
const BUTTON_NORMAL: Color = Color::srgb(0.15, 0.15, 0.15);
const BUTTON_HOVERED: Color = Color::srgb(0.22, 0.24, 0.28);
const BUTTON_PRESSED: Color = Color::srgb(0.08, 0.30, 0.18);
const LEVEL_BUTTON_NORMAL: Color = Color::srgb(0.12, 0.14, 0.16);
const LEVEL_BUTTON_HOVERED: Color = Color::srgb(0.22, 0.24, 0.28);
const LEVEL_BUTTON_PRESSED: Color = Color::srgb(0.12, 0.26, 0.38);
const LEVEL_BUTTON_ACTIVE: Color = Color::srgb(0.18, 0.40, 0.28);

#[derive(Component)]
pub(crate) struct HudText;

#[derive(Component)]
pub(crate) struct HealButton;

#[derive(Component)]
pub(crate) struct LevelButton {
  level: GameLevel,
}

#[derive(Component)]
pub(crate) struct LocationText;

#[derive(Component)]
pub(crate) struct ActorLabel {
  target: Entity,
}

pub(crate) fn spawn_actor_label(
  commands: &mut Commands,
  target: Entity,
  caption: &str,
  level: GameLevel,
) {
  commands.spawn((
    Text2d::new(caption),
    TextFont {
      font_size: FontSize::Px(18.0),
      ..default()
    },
    TextColor(Color::WHITE),
    TextLayout::justify(Justify::Center),
    TextShadow::default(),
    Transform::from_xyz(0.0, 0.0, 3.0),
    ActorLabel { target },
    LevelEntity,
    DespawnOnExit(level),
  ));
}

pub(crate) fn spawn_heal_button(commands: &mut Commands) {
  commands
    .spawn((
      Button,
      Hovered::default(),
      Node {
        position_type: PositionType::Absolute,
        top: px(14.0),
        right: px(16.0),
        width: px(150.0),
        height: px(52.0),
        border: UiRect::all(px(3.0)),
        border_radius: BorderRadius::all(px(8.0)),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        ..default()
      },
      BorderColor::from(Color::BLACK),
      BackgroundColor(BUTTON_NORMAL),
      HealButton,
    ))
    .with_children(|button| {
      button.spawn((
        Text::new("Heal"),
        TextFont {
          font_size: FontSize::Px(24.0),
          ..default()
        },
        TextColor(Color::srgb(0.9, 0.9, 0.9)),
        TextShadow::default(),
      ));
    });
}

pub(crate) fn spawn_level_buttons(commands: &mut Commands) {
  for (index, level) in all_levels().into_iter().enumerate() {
    let map = level.map();
    commands
      .spawn((
        Button,
        Hovered::default(),
        Node {
          position_type: PositionType::Absolute,
          top: px(56.0),
          left: px(16.0 + index as f32 * 154.0),
          width: px(146.0),
          height: px(44.0),
          border: UiRect::all(px(2.0)),
          border_radius: BorderRadius::all(px(8.0)),
          justify_content: JustifyContent::Center,
          align_items: AlignItems::Center,
          ..default()
        },
        BorderColor::from(Color::BLACK),
        BackgroundColor(LEVEL_BUTTON_NORMAL),
        LevelButton { level },
      ))
      .with_children(|button| {
        button.spawn((
          Text::new(format!("L{} {}", level_number(level), map.name)),
          TextFont {
            font_size: FontSize::Px(16.0),
            ..default()
          },
          TextColor(Color::srgb(0.92, 0.94, 0.96)),
          TextShadow::default(),
        ));
      });
  }
}

pub(crate) fn spawn_location_text(commands: &mut Commands) {
  commands.spawn((
    Text::new("Map loading..."),
    TextFont {
      font_size: FontSize::Px(18.0),
      ..default()
    },
    TextColor(Color::srgb(0.9, 0.92, 0.86)),
    TextShadow::default(),
    Node {
      position_type: PositionType::Absolute,
      top: px(108.0),
      left: px(16.0),
      ..default()
    },
    LocationText,
  ));
}

pub(crate) fn heal_button_interactions(
  pressed: Query<(), (With<HealButton>, Added<Pressed>)>,
  mut buttons: Query<(&Hovered, Has<Pressed>, &mut BackgroundColor), With<HealButton>>,
  mut players: Query<&mut RedBlueValues, With<Player>>,
) {
  if !pressed.is_empty() {
    if let Ok(mut values) = players.single_mut() {
      values.blue = HEAL_AMOUNT;
    }
  }
  for (hovered, pressed, mut color) in &mut buttons {
    *color = if pressed {
      BUTTON_PRESSED
    } else if hovered.0 {
      BUTTON_HOVERED
    } else {
      BUTTON_NORMAL
    }
    .into();
  }
}

pub(crate) fn level_button_interactions(
  buttons: Query<&LevelButton, Added<Pressed>>,
  mut next_level: ResMut<NextState<GameLevel>>,
) {
  for button in &buttons {
    NextState::set_if_different(next_level.as_mut(), button.level);
  }
}

pub(crate) fn update_level_button_styles(
  level: Res<State<GameLevel>>,
  mut buttons: Query<(&LevelButton, &Hovered, Has<Pressed>, &mut BackgroundColor)>,
) {
  let current = *level.get();
  for (button, hovered, pressed, mut color) in &mut buttons {
    *color = level_button_color(button.level, current, hovered.0, pressed).into();
  }
}

pub(crate) fn update_actor_labels(
  mut commands: Commands,
  actors: Query<(
    &ActorKind,
    &ArenaPosition,
    &RedBlueValues,
    Option<&MaxHealth>,
  )>,
  mut labels: Query<(
    Entity,
    &ActorLabel,
    &mut Text2d,
    &mut TextColor,
    &mut Transform,
  )>,
) {
  for (label_entity, label, mut text, mut color, mut transform) in &mut labels {
    let Ok((kind, position, values, max_health)) = actors.get(label.target) else {
      commands.entity(label_entity).despawn();
      continue;
    };

    let name = match kind {
      ActorKind::Player => "Player",
      ActorKind::Monster => "Monster",
    };

    let max_b = max_health.map_or(140, |m| m.0).max(1);
    let ratio = (values.blue as f32 / max_b as f32).clamp(0.0, 1.0);
    let bars = 10;
    let filled = (ratio * bars as f32).round() as usize;
    let empty = bars - filled;
    let bar: String = (0 .. filled)
      .map(|_| '█')
      .chain((0 .. empty).map(|_| '░'))
      .collect();

    text.0 = format!("{name} R{} {bar} B{}", values.red, values.blue);
    *color = if ratio > 0.5 {
      TextColor(Color::srgb(0.4, 0.9, 0.4))
    } else if ratio > 0.25 {
      TextColor(Color::srgb(0.9, 0.9, 0.3))
    } else {
      TextColor(Color::srgb(0.9, 0.3, 0.3))
    };
    transform.translation = game_to_world_position(position.0, 2.4);
  }
}

pub(crate) fn update_location_text(
  level: Res<State<GameLevel>>,
  map: Res<LevelMap>,
  player_query: Query<&ArenaPosition, With<Player>>,
  mut labels: Query<&mut Text, With<LocationText>>,
) {
  let Ok(player_position) = player_query.single() else {
    return;
  };
  let line = format!(
    "Map {} of {}: {} | Roadblocks {} | Player X {:.0} Y {:.0}",
    level_number(*level.get()),
    level_total(),
    map.name,
    map.obstacles.len(),
    player_position.0.x,
    player_position.0.y
  );

  for mut text in &mut labels {
    text.0 = line.clone();
  }
}

pub(crate) fn update_hud(
  level: Res<State<GameLevel>>,
  map: Res<LevelMap>,
  player_query: Query<(&RedBlueValues, Option<&PlayerDefeated>), With<Player>>,
  monsters: Query<&RedBlueValues, With<Monster>>,
  mut hud: Query<&mut Text, With<HudText>>,
  mut window: Single<&mut Window>,
) {
  let Ok((player_values, player_defeated)) = player_query.single() else {
    return;
  };

  let alive_monsters = monsters.iter().filter(|values| values.blue > 0).count();
  let status = if player_defeated.is_some() {
    "Player defeated"
  } else if alive_monsters == 0 {
    "All monsters defeated"
  } else {
    "Fighting"
  };
  let line = format!(
    "Level {}/{} {} ({}) | Roadblocks {} | WASD move | 1-{} switch | Player R{} B{} | Monsters {} \
     | {}",
    level_number(*level.get()),
    level_total(),
    map.name,
    map.hint,
    map.obstacles.len(),
    level_total(),
    player_values.red,
    player_values.blue,
    alive_monsters,
    status
  );

  window.title = line.clone();
  for mut text in &mut hud {
    text.0 = line.clone();
  }
}

fn level_button_color(level: GameLevel, current: GameLevel, hovered: bool, pressed: bool) -> Color {
  if level == current {
    return LEVEL_BUTTON_ACTIVE;
  }

  if pressed {
    LEVEL_BUTTON_PRESSED
  } else if hovered {
    LEVEL_BUTTON_HOVERED
  } else {
    LEVEL_BUTTON_NORMAL
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn heal_button_heals_once_per_press_and_updates_colors_on_release() {
    let mut app = App::new();
    app.add_systems(Update, heal_button_interactions);
    let player = app
      .world_mut()
      .spawn((Player, RedBlueValues { red: 18, blue: 0 }))
      .id();
    let button = app
      .world_mut()
      .spawn((
        HealButton,
        Hovered(true),
        Pressed,
        BackgroundColor(BUTTON_NORMAL),
      ))
      .id();
    app.update();
    assert_eq!(
      app.world().get::<RedBlueValues>(player).unwrap().blue,
      HEAL_AMOUNT
    );
    assert_eq!(
      app.world().get::<BackgroundColor>(button).unwrap().0,
      BUTTON_PRESSED
    );
    app
      .world_mut()
      .get_mut::<RedBlueValues>(player)
      .unwrap()
      .blue = 10;
    app.update();
    assert_eq!(app.world().get::<RedBlueValues>(player).unwrap().blue, 10);
    app.world_mut().entity_mut(button).remove::<Pressed>();
    app.update();
    assert_eq!(
      app.world().get::<BackgroundColor>(button).unwrap().0,
      BUTTON_HOVERED
    );
    app
      .world_mut()
      .entity_mut(button)
      .insert((Hovered(false), Pressed));
    app.update();
    assert_eq!(
      app.world().get::<RedBlueValues>(player).unwrap().blue,
      HEAL_AMOUNT
    );
    app.world_mut().entity_mut(button).remove::<Pressed>();
    app.update();
    assert_eq!(
      app.world().get::<BackgroundColor>(button).unwrap().0,
      BUTTON_NORMAL
    );
  }

  #[test]
  fn level_button_requests_selected_level_on_press() {
    let mut app = App::new();
    app.init_resource::<NextState<GameLevel>>();
    app.add_systems(Update, level_button_interactions);
    app.world_mut().spawn((
      LevelButton {
        level: GameLevel::Citadel,
      },
      Pressed,
    ));
    app.update();
    assert!(matches!(
      app.world().resource::<NextState<GameLevel>>(),
      NextState::PendingIfDifferent(GameLevel::Citadel)
    ));
  }
}
