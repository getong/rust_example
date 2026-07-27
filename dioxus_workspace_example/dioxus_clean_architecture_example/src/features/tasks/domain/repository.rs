use thiserror::Error;

use super::{Task, TaskId};

pub type RepositoryResult<T> = Result<T, RepositoryError>;

#[derive(Debug, Error)]
pub enum RepositoryError {
  #[error("storage operation failed: {0}")]
  Storage(String),
  #[error("stored task data is invalid: {0}")]
  InvalidData(String),
}

pub trait TaskRepository: Send + Sync {
  fn all(&self) -> RepositoryResult<Vec<Task>>;
  fn save(&self, task: &Task) -> RepositoryResult<()>;
  fn delete(&self, id: TaskId) -> RepositoryResult<()>;
  fn clear_completed(&self) -> RepositoryResult<()>;
}
