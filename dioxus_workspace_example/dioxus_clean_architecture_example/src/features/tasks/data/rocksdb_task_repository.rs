use std::{cmp::Reverse, path::Path};

use rocksdb::{DB, IteratorMode, Options, WriteBatch};

use crate::features::tasks::domain::{
  RepositoryError, RepositoryResult, Task, TaskId, TaskRepository,
};

const TASK_KEY_PREFIX: &str = "task:";

pub struct RocksDbTaskRepository {
  database: DB,
}

impl RocksDbTaskRepository {
  pub fn open(path: &Path) -> RepositoryResult<Self> {
    let mut options = Options::default();
    options.create_if_missing(true);

    DB::open(&options, path)
      .map(|database| Self { database })
      .map_err(storage_error)
  }

  fn key(id: TaskId) -> String {
    format!("{TASK_KEY_PREFIX}{id}")
  }
}

impl TaskRepository for RocksDbTaskRepository {
  fn all(&self) -> RepositoryResult<Vec<Task>> {
    let mut tasks = Vec::new();

    for entry in self.database.iterator(IteratorMode::Start) {
      let (key, value) = entry.map_err(storage_error)?;
      if !key.starts_with(TASK_KEY_PREFIX.as_bytes()) {
        continue;
      }

      let task: Task = serde_json::from_slice(&value)
        .map_err(|error| RepositoryError::InvalidData(error.to_string()))?;
      tasks.push(task);
    }

    tasks.sort_unstable_by_key(|task| Reverse(task.created_at));
    Ok(tasks)
  }

  fn save(&self, task: &Task) -> RepositoryResult<()> {
    let value =
      serde_json::to_vec(task).map_err(|error| RepositoryError::InvalidData(error.to_string()))?;

    self
      .database
      .put(Self::key(task.id), value)
      .map_err(storage_error)
  }

  fn delete(&self, id: TaskId) -> RepositoryResult<()> {
    self.database.delete(Self::key(id)).map_err(storage_error)
  }

  fn clear_completed(&self) -> RepositoryResult<()> {
    let mut batch = WriteBatch::default();
    for task in self.all()?.into_iter().filter(|task| task.completed) {
      batch.delete(Self::key(task.id));
    }

    self.database.write(batch).map_err(storage_error)
  }
}

fn storage_error(error: rocksdb::Error) -> RepositoryError {
  RepositoryError::Storage(error.to_string())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn tasks_survive_database_reopen() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let database_path = directory.path().join("tasks.rocksdb");
    let task = Task::new("Persist this task");

    {
      let repository = RocksDbTaskRepository::open(&database_path).expect("database should open");
      repository.save(&task).expect("task should be saved");
    }

    let repository = RocksDbTaskRepository::open(&database_path).expect("database should reopen");
    let stored_tasks = repository.all().expect("tasks should be loaded");

    assert_eq!(stored_tasks, vec![task]);
  }
}
