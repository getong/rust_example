use bevy::prelude::*;
use seldom_state::prelude::*;

use crate::actors::{Player, RedBlueValues};

#[derive(Clone, Component, Debug)]
#[component(storage = "SparseSet")]
pub(crate) struct PlayerActive;

#[derive(Clone, Component, Debug)]
#[component(storage = "SparseSet")]
pub(crate) struct PlayerDefeated;

pub(crate) fn player_state_machine() -> StateMachine {
  StateMachine::default()
    .trans::<PlayerActive, _>(player_is_defeated, PlayerDefeated)
    .trans::<PlayerDefeated, _>(player_is_active, PlayerActive)
}

fn player_is_defeated(
  In(entity): In<Entity>,
  players: Query<&RedBlueValues, With<Player>>,
) -> bool {
  players.get(entity).is_ok_and(|values| values.blue <= 0)
}

fn player_is_active(In(entity): In<Entity>, players: Query<&RedBlueValues, With<Player>>) -> bool {
  players.get(entity).is_ok_and(|values| values.blue > 0)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn player_transitions_to_defeated_when_health_reaches_zero() {
    let mut app = App::new();
    app.add_plugins(StateMachinePlugin::default().schedule(Update));

    let player = app
      .world_mut()
      .spawn((
        Player,
        RedBlueValues { red: 18, blue: 1 },
        PlayerActive,
        player_state_machine(),
      ))
      .id();

    app.update();
    assert!(app.world().entity(player).contains::<PlayerActive>());
    assert!(!app.world().entity(player).contains::<PlayerDefeated>());

    app
      .world_mut()
      .entity_mut(player)
      .insert(RedBlueValues { red: 18, blue: 0 });

    app.update();
    assert!(!app.world().entity(player).contains::<PlayerActive>());
    assert!(app.world().entity(player).contains::<PlayerDefeated>());
  }

  #[test]
  fn player_transitions_back_to_active_when_health_is_restored() {
    let mut app = App::new();
    app.add_plugins(StateMachinePlugin::default().schedule(Update));

    let player = app
      .world_mut()
      .spawn((
        Player,
        RedBlueValues { red: 18, blue: 0 },
        PlayerDefeated,
        player_state_machine(),
      ))
      .id();

    app.update();
    assert!(app.world().entity(player).contains::<PlayerDefeated>());

    app
      .world_mut()
      .entity_mut(player)
      .insert(RedBlueValues { red: 18, blue: 10 });

    app.update();
    assert!(app.world().entity(player).contains::<PlayerActive>());
    assert!(!app.world().entity(player).contains::<PlayerDefeated>());
  }
}
