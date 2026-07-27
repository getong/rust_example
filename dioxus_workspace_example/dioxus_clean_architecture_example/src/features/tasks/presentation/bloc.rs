use std::{path::PathBuf, sync::Arc};

use dioxus::prelude::*;

use crate::features::tasks::{
  data::RocksDbTaskRepository,
  domain::{RepositoryError, Task, TaskId, TaskRepository},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TaskFilter {
  #[default]
  All,
  Active,
  Completed,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TaskState {
  pub tasks: Vec<Task>,
  pub filter: TaskFilter,
  pub error: Option<String>,
}

impl TaskState {
  #[must_use]
  pub fn visible_tasks(&self) -> Vec<Task> {
    self
      .tasks
      .iter()
      .filter(|task| match self.filter {
        TaskFilter::All => true,
        TaskFilter::Active => !task.completed,
        TaskFilter::Completed => task.completed,
      })
      .cloned()
      .collect()
  }

  #[must_use]
  pub fn completed_count(&self) -> usize {
    self.tasks.iter().filter(|task| task.completed).count()
  }

  #[must_use]
  pub fn active_count(&self) -> usize {
    self.tasks.len() - self.completed_count()
  }

  #[must_use]
  pub fn completion_percent(&self) -> usize {
    if self.tasks.is_empty() {
      0
    } else {
      self.completed_count() * 100 / self.tasks.len()
    }
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TaskEvent {
  Add(String),
  Toggle(TaskId),
  Delete(TaskId),
  SetFilter(TaskFilter),
  ClearCompleted,
  DismissError,
}

#[derive(Clone)]
pub struct TaskBloc {
  repository: Option<Arc<dyn TaskRepository>>,
  state: Signal<TaskState>,
  database_path: PathBuf,
}

impl TaskBloc {
  #[must_use]
  pub fn open(database_path: PathBuf) -> Self {
    match RocksDbTaskRepository::open(&database_path) {
      Ok(repository) => {
        let controller = TaskController::with_repository(Arc::new(repository));
        Self::from_controller(controller, database_path)
      }
      Err(error) => Self {
        repository: None,
        state: Signal::new(TaskState {
          error: Some(error.to_string()),
          ..TaskState::default()
        }),
        database_path,
      },
    }
  }

  fn from_controller(controller: TaskController, database_path: PathBuf) -> Self {
    Self {
      repository: controller.repository,
      state: Signal::new(controller.state),
      database_path,
    }
  }

  #[must_use]
  pub fn snapshot(&self) -> TaskState {
    self.state.read().clone()
  }

  #[must_use]
  pub fn database_path(&self) -> &PathBuf {
    &self.database_path
  }

  #[must_use]
  pub fn is_available(&self) -> bool {
    self.repository.is_some()
  }

  pub fn dispatch(&mut self, event: TaskEvent) {
    let mut controller = TaskController {
      repository: self.repository.clone(),
      state: self.state.peek().clone(),
    };
    controller.dispatch(event);
    self.state.set(controller.state);
  }
}

struct TaskController {
  repository: Option<Arc<dyn TaskRepository>>,
  state: TaskState,
}

impl TaskController {
  fn with_repository(repository: Arc<dyn TaskRepository>) -> Self {
    let mut controller = Self {
      repository: Some(repository),
      state: TaskState::default(),
    };
    controller.reload();
    controller
  }

  fn dispatch(&mut self, event: TaskEvent) {
    match event {
      TaskEvent::SetFilter(filter) => self.state.filter = filter,
      TaskEvent::DismissError => self.state.error = None,
      TaskEvent::Add(title) => self.add(title),
      TaskEvent::Toggle(id) => self.toggle(id),
      TaskEvent::Delete(id) => self.run_repository_action(|repository| repository.delete(id)),
      TaskEvent::ClearCompleted => {
        self.run_repository_action(|repository| repository.clear_completed());
      }
    }
  }

  fn add(&mut self, title: String) {
    if title.trim().is_empty() {
      return;
    }

    let task = Task::new(title);
    self.run_repository_action(|repository| repository.save(&task));
  }

  fn toggle(&mut self, id: TaskId) {
    let task = self.state.tasks.iter().find(|task| task.id == id).cloned();

    if let Some(mut task) = task {
      task.toggle();
      self.run_repository_action(|repository| repository.save(&task));
    }
  }

  fn run_repository_action(
    &mut self,
    action: impl FnOnce(&dyn TaskRepository) -> Result<(), RepositoryError>,
  ) {
    let result = self
      .repository
      .as_deref()
      .ok_or_else(|| RepositoryError::Storage("database is unavailable".to_owned()))
      .and_then(action);

    match result {
      Ok(()) => self.reload(),
      Err(error) => self.state.error = Some(error.to_string()),
    }
  }

  fn reload(&mut self) {
    let result = self
      .repository
      .as_deref()
      .ok_or_else(|| RepositoryError::Storage("database is unavailable".to_owned()))
      .and_then(TaskRepository::all);

    match result {
      Ok(tasks) => {
        self.state.tasks = tasks;
        self.state.error = None;
      }
      Err(error) => self.state.error = Some(error.to_string()),
    }
  }
}

pub fn use_task_bloc() -> TaskBloc {
  use_context::<TaskBloc>()
}

#[cfg(test)]
mod tests {
  use std::sync::Mutex;

  use super::*;
  use crate::features::tasks::domain::RepositoryResult;

  #[derive(Default)]
  struct MemoryTaskRepository {
    tasks: Mutex<Vec<Task>>,
  }

  impl TaskRepository for MemoryTaskRepository {
    fn all(&self) -> RepositoryResult<Vec<Task>> {
      Ok(
        self
          .tasks
          .lock()
          .expect("task lock should not be poisoned")
          .clone(),
      )
    }

    fn save(&self, task: &Task) -> RepositoryResult<()> {
      let mut tasks = self.tasks.lock().expect("task lock should not be poisoned");
      if let Some(existing) = tasks.iter_mut().find(|existing| existing.id == task.id) {
        existing.clone_from(task);
      } else {
        tasks.push(task.clone());
      }
      Ok(())
    }

    fn delete(&self, id: TaskId) -> RepositoryResult<()> {
      self
        .tasks
        .lock()
        .expect("task lock should not be poisoned")
        .retain(|task| task.id != id);
      Ok(())
    }

    fn clear_completed(&self) -> RepositoryResult<()> {
      self
        .tasks
        .lock()
        .expect("task lock should not be poisoned")
        .retain(|task| !task.completed);
      Ok(())
    }
  }

  #[test]
  fn add_toggle_filter_and_delete_flow() {
    let repository = Arc::new(MemoryTaskRepository::default());
    let mut controller = TaskController::with_repository(repository);

    controller.dispatch(TaskEvent::Add("Write tests".to_owned()));
    let task_id = controller.state.tasks[0].id;
    controller.dispatch(TaskEvent::Toggle(task_id));
    controller.dispatch(TaskEvent::SetFilter(TaskFilter::Completed));

    assert_eq!(controller.state.visible_tasks().len(), 1);
    assert_eq!(controller.state.completion_percent(), 100);

    controller.dispatch(TaskEvent::Delete(task_id));

    assert!(controller.state.tasks.is_empty());
  }
}
