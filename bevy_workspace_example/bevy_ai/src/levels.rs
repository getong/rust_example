use bevy::prelude::*;
use bevy_state::prelude::{NextState, State, States};

use crate::{
  actors::{Monster, Player, RedBlueValues},
  player_state::PlayerDefeated,
  terrain::{LevelMap, level_count, level_map},
};

#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum GameLevel {
  #[default]
  Crossroads,
  Switchback,
  Citadel,
  Maze,
  Amphitheater,
  Ravine,
}

#[derive(Resource, Default, Debug, Clone, Copy)]
pub(crate) struct LoadedLevel {
  pub(crate) current: Option<GameLevel>,
}

#[derive(Component)]
pub(crate) struct LevelEntity;

const GAME_LEVELS: [GameLevel; 6] = [
  GameLevel::Crossroads,
  GameLevel::Switchback,
  GameLevel::Citadel,
  GameLevel::Maze,
  GameLevel::Amphitheater,
  GameLevel::Ravine,
];

impl GameLevel {
  pub(crate) fn map(self) -> LevelMap {
    level_map(self.index())
  }

  fn index(self) -> usize {
    match self {
      Self::Crossroads => 0,
      Self::Switchback => 1,
      Self::Citadel => 2,
      Self::Maze => 3,
      Self::Amphitheater => 4,
      Self::Ravine => 5,
    }
  }

  fn next(self) -> Option<Self> {
    match self {
      Self::Crossroads => Some(Self::Switchback),
      Self::Switchback => Some(Self::Citadel),
      Self::Citadel => Some(Self::Maze),
      Self::Maze => Some(Self::Amphitheater),
      Self::Amphitheater => Some(Self::Ravine),
      Self::Ravine => None,
    }
  }

  fn from_digit(keyboard: &ButtonInput<KeyCode>) -> Option<Self> {
    if keyboard.just_pressed(KeyCode::Digit1) {
      Some(Self::Crossroads)
    } else if keyboard.just_pressed(KeyCode::Digit2) {
      Some(Self::Switchback)
    } else if keyboard.just_pressed(KeyCode::Digit3) {
      Some(Self::Citadel)
    } else if keyboard.just_pressed(KeyCode::Digit4) {
      Some(Self::Maze)
    } else if keyboard.just_pressed(KeyCode::Digit5) {
      Some(Self::Amphitheater)
    } else if keyboard.just_pressed(KeyCode::Digit6) {
      Some(Self::Ravine)
    } else {
      None
    }
  }
}

pub(crate) fn all_levels() -> [GameLevel; 6] {
  GAME_LEVELS
}

pub(crate) fn level_input(
  keyboard: Res<ButtonInput<KeyCode>>,
  mut next_level: ResMut<NextState<GameLevel>>,
) {
  if let Some(level) = GameLevel::from_digit(&keyboard) {
    NextState::set_if_neq(next_level.as_mut(), level);
  }
}

pub(crate) fn advance_when_level_cleared(
  current_level: Res<State<GameLevel>>,
  monsters: Query<&RedBlueValues, With<Monster>>,
  player: Query<Option<&PlayerDefeated>, With<Player>>,
  mut next_level: ResMut<NextState<GameLevel>>,
) {
  let Ok(player_defeated) = player.single() else {
    return;
  };
  if player_defeated.is_some() {
    return;
  }

  if monsters.iter().any(|values| values.blue > 0) {
    return;
  }

  if let Some(level) = current_level.get().next() {
    NextState::set(next_level.as_mut(), level);
  }
}

pub(crate) fn level_number(level: GameLevel) -> usize {
  level.index() + 1
}

pub(crate) fn level_total() -> usize {
  level_count()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn levels_advance_in_order() {
    assert_eq!(GameLevel::Crossroads.next(), Some(GameLevel::Switchback));
    assert_eq!(GameLevel::Switchback.next(), Some(GameLevel::Citadel));
    assert_eq!(GameLevel::Citadel.next(), Some(GameLevel::Maze));
    assert_eq!(GameLevel::Maze.next(), Some(GameLevel::Amphitheater));
    assert_eq!(GameLevel::Amphitheater.next(), Some(GameLevel::Ravine));
    assert_eq!(GameLevel::Ravine.next(), None);
  }

  #[test]
  fn each_level_has_a_map_definition() {
    for level in all_levels() {
      let map = level.map();
      assert_eq!(map.index as usize, level.index());
      assert!(!map.monsters.is_empty());
      assert!(!map.obstacles.is_empty());
    }
  }
}
