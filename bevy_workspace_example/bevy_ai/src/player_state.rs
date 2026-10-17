use bevy::prelude::*;
use statig::prelude::{IntoStateMachineExt, Outcome, StateMachine, state_machine};

use crate::actors::{Player, RedBlueValues};

#[derive(Clone, Component, Debug)]
#[component(storage = "SparseSet")]
pub(crate) struct PlayerActive;

#[derive(Clone, Component, Debug)]
#[component(storage = "SparseSet")]
pub(crate) struct PlayerDefeated;

// The wrapper uses this application's Bevy version; statig itself is engine-independent.
#[derive(Component)]
pub(crate) struct PlayerStateMachine(StateMachine<PlayerLife>);

#[derive(Default)]
pub(crate) struct PlayerLife;

#[state_machine(initial = "LifeState::active()", state(name = "LifeState"))]
impl PlayerLife {
  #[state]
  fn active(event: &i32) -> Outcome<LifeState> {
    if *event <= 0 {
      Outcome::Transition(LifeState::defeated())
    } else {
      Outcome::Handled
    }
  }

  #[state]
  fn defeated(event: &i32) -> Outcome<LifeState> {
    if *event > 0 {
      Outcome::Transition(LifeState::active())
    } else {
      Outcome::Handled
    }
  }
}

pub(crate) fn player_state_machine() -> PlayerStateMachine {
  PlayerStateMachine(PlayerLife.state_machine())
}

pub(crate) fn transition_player_state(
  mut commands: Commands,
  mut players: Query<
    (
      Entity,
      &RedBlueValues,
      &mut PlayerStateMachine,
      Has<PlayerActive>,
      Has<PlayerDefeated>,
    ),
    With<Player>,
  >,
) {
  for (entity, values, mut machine, active, defeated) in &mut players {
    machine.0.handle(&values.blue);
    match machine.0.state() {
      LifeState::Active {} if !active || defeated => {
        commands
          .entity(entity)
          .remove::<PlayerDefeated>()
          .insert(PlayerActive);
      }
      LifeState::Defeated {} if !defeated || active => {
        commands
          .entity(entity)
          .remove::<PlayerActive>()
          .insert(PlayerDefeated);
      }
      _ => {}
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn entity_states_are_independent_and_visible_to_later_systems() {
    let mut app = App::new();
    app.add_systems(Update, (transition_player_state, assert_states).chain());
    for blue in [-10, 0, 1, 140] {
      app.world_mut().spawn((
        Player,
        RedBlueValues { red: 18, blue },
        player_state_machine(),
      ));
    }
    app.update();
    app.update();

    fn assert_states(players: Query<(&RedBlueValues, Has<PlayerActive>, Has<PlayerDefeated>)>) {
      assert_eq!(players.iter().count(), 4);
      for (values, active, defeated) in &players {
        assert_eq!(active, values.blue > 0);
        assert_eq!(defeated, values.blue <= 0);
      }
    }
  }

  #[test]
  fn player_transitions_to_defeated_when_health_reaches_zero() {
    let mut app = App::new();
    app.add_systems(Update, transition_player_state);

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
    app.add_systems(Update, transition_player_state);

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
