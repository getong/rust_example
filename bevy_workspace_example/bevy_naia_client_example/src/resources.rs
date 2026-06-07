use std::default::Default;

use bevy::prelude::{Entity, Resource};
use naia_bevy_client::CommandHistory;
use naia_bevy_demo_shared::messages::KeyCommand;

pub struct OwnedEntity {
  pub confirmed: Entity,
  pub predicted: Entity,
}

impl OwnedEntity {
  pub fn new(confirmed_entity: Entity, predicted_entity: Entity) -> Self {
    OwnedEntity {
      confirmed: confirmed_entity,
      predicted: predicted_entity,
    }
  }
}

#[derive(Resource)]
pub struct Global {
  pub owned_entity: Option<OwnedEntity>,
  pub cursor_entity: Option<Entity>,
  pub queued_command: Option<KeyCommand>,
  pub command_history: CommandHistory<KeyCommand>,
}

impl Default for Global {
  fn default() -> Self {
    Self {
      owned_entity: None,
      cursor_entity: None,
      queued_command: None,
      command_history: CommandHistory::default(),
    }
  }
}
