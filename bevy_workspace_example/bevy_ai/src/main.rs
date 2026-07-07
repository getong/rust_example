mod actors;
mod config;
mod gameplay;
mod lighting;
mod monster_behavior;
mod plugin;
mod setup;
mod terrain;
mod ui;

use bevy::{prelude::*, window::PresentMode};

use crate::plugin::BevyAiPlugin;

fn main() {
  App::new()
    .add_plugins(DefaultPlugins.set(WindowPlugin {
      primary_window: Some(Window {
        title: "Bevy AI - WASD to move".into(),
        resolution: (960, 640).into(),
        present_mode: PresentMode::AutoVsync,
        ..default()
      }),
      ..default()
    }))
    .add_plugins(BevyAiPlugin)
    .run();
}
