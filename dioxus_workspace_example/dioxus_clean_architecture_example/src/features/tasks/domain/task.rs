use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskId(Uuid);

impl TaskId {
  #[must_use]
  pub fn new() -> Self {
    Self(Uuid::new_v4())
  }
}

impl Default for TaskId {
  fn default() -> Self {
    Self::new()
  }
}

impl std::fmt::Display for TaskId {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    self.0.fmt(formatter)
  }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
  pub id: TaskId,
  pub title: String,
  pub completed: bool,
  pub created_at: u64,
}

impl Task {
  #[must_use]
  pub fn new(title: impl Into<String>) -> Self {
    Self {
      id: TaskId::new(),
      title: title.into().trim().to_owned(),
      completed: false,
      created_at: current_timestamp(),
    }
  }

  pub fn toggle(&mut self) {
    self.completed = !self.completed;
  }
}

fn current_timestamp() -> u64 {
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map_or(0, |duration| duration.as_secs())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn new_task_trims_title_and_starts_active() {
    let task = Task::new("  Ship the feature  ");

    assert_eq!(task.title, "Ship the feature");
    assert!(!task.completed);
  }

  #[test]
  fn toggle_changes_completion_state() {
    let mut task = Task::new("Review tests");

    task.toggle();

    assert!(task.completed);
  }
}
