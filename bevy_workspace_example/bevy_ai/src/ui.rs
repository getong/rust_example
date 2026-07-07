use bevy::prelude::*;

use crate::{
  actors::{ActorKind, ArenaPosition, Monster, Player, RedBlueValues},
  player_state::PlayerDefeated,
  terrain::game_to_world_position,
};

#[derive(Component)]
pub(crate) struct HudText;

#[derive(Component)]
pub(crate) struct ActorLabel {
  target: Entity,
}

pub(crate) fn spawn_actor_label(commands: &mut Commands, target: Entity, caption: &str) {
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
  ));
}

pub(crate) fn update_actor_labels(
  mut commands: Commands,
  actors: Query<(&ActorKind, &ArenaPosition, &RedBlueValues)>,
  mut labels: Query<(Entity, &ActorLabel, &mut Text2d, &mut Transform)>,
) {
  for (label_entity, label, mut text, mut transform) in &mut labels {
    let Ok((kind, position, values)) = actors.get(label.target) else {
      commands.entity(label_entity).despawn();
      continue;
    };

    let name = match kind {
      ActorKind::Player => "Player",
      ActorKind::Monster => "Monster",
    };

    text.0 = format!("{name} R{} B{}", values.red, values.blue);
    transform.translation = game_to_world_position(position.0, 2.4);
  }
}

pub(crate) fn update_hud(
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
    "WASD: move | Red: attack | Blue: health | Player R{} B{} | Monsters {} | {}",
    player_values.red, player_values.blue, alive_monsters, status
  );

  window.title = line.clone();
  for mut text in &mut hud {
    text.0 = line.clone();
  }
}
