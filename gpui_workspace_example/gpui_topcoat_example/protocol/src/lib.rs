//! Shared wire format; independent of either UI framework.
use serde::{Deserialize, Serialize};

pub const DEFAULT_SERVER_URL: &str = "http://127.0.0.1:3000";
pub const COUNTER_PATH: &str = "/api/counter";

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct CounterSnapshot {
  pub value: u64,
  pub revision: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CounterAction {
  Increment,
  Reset,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateCounter {
  pub action: CounterAction,
}

pub const DEMOS_PATH: &str = "/api/demos";
pub const TODOS_PATH: &str = "/api/todos";
pub const ECHO_PATH: &str = "/api/echo";
pub const PROFILE_PATH: &str = "/api/profile";

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Todo {
  pub id: u64,
  pub title: String,
  pub done: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum TodoCommand {
  Create { title: String },
  SetDone { id: u64, done: bool },
  Delete { id: u64 },
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Profile {
  pub username: String,
  pub age: u8,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct DemoSnapshot {
  pub todos: Vec<Todo>,
  pub echoes: Vec<serde_json::Value>,
  pub profiles: Vec<Profile>,
}

pub const STUDIO_PATH: &str = "/api/studio";
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StudioTheme {
  #[default]
  Ocean,
  Sunset,
  Forest,
}
impl StudioTheme {
  pub fn label(self) -> &'static str {
    match self {
      Self::Ocean => "海洋蓝",
      Self::Sunset => "落日橙",
      Self::Forest => "森林绿",
    }
  }
  pub fn color(self) -> u32 {
    match self {
      Self::Ocean => 0x2563eb,
      Self::Sunset => 0xea580c,
      Self::Forest => 0x16a34a,
    }
  }
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct StudioSnapshot {
  pub theme: StudioTheme,
  pub intensity: u8,
  pub revision: u32,
}
impl Default for StudioSnapshot {
  fn default() -> Self {
    Self {
      theme: StudioTheme::Ocean,
      intensity: 70,
      revision: 0,
    }
  }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum StudioCommand {
  Apply { theme: StudioTheme, intensity: u8 },
}

pub mod crypto;
