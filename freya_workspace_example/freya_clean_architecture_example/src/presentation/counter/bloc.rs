use std::rc::Rc;

use freya::radio::{ChannelSelection, DataReducer, RadioChannel};

use crate::domain::{
  entities::Counter,
  repositories::CounterRepository,
  use_cases::{DecrementCounter, IncrementCounter, LoadCounter, SaveCounter},
};

pub(crate) struct CounterState {
  counter: Counter,
  repository: Rc<dyn CounterRepository>,
  persistence_status: PersistenceStatus,
  storage_location: String,
}

impl CounterState {
  pub(crate) fn restore(
    default_counter: Counter,
    repository: Rc<dyn CounterRepository>,
    storage_location: String,
  ) -> Self {
    let (counter, persistence_status) = match LoadCounter::new(repository.as_ref()).execute() {
      Ok(Some(counter)) => (counter, PersistenceStatus::Loaded),
      Ok(None) => {
        let status = Self::save_status(repository.as_ref(), &default_counter);
        (default_counter, status)
      }
      Err(error) => (
        default_counter,
        PersistenceStatus::Failed(error.to_string()),
      ),
    };

    Self {
      counter,
      repository,
      persistence_status,
      storage_location,
    }
  }

  pub(crate) const fn count(&self) -> i32 {
    self.counter.value()
  }

  pub(crate) fn persistence_status(&self) -> &PersistenceStatus {
    &self.persistence_status
  }

  pub(crate) fn storage_location(&self) -> &str {
    &self.storage_location
  }

  fn persist(&mut self) {
    self.persistence_status = Self::save_status(self.repository.as_ref(), &self.counter);
  }

  fn save_status(repository: &dyn CounterRepository, counter: &Counter) -> PersistenceStatus {
    match SaveCounter::new(repository).execute(counter) {
      Ok(()) => PersistenceStatus::Saved,
      Err(error) => PersistenceStatus::Failed(error.to_string()),
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CounterEvent {
  Increment,
  Decrement,
  SaveNow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum CounterChannel {
  Count,
  Persistence,
}

impl RadioChannel<CounterState> for CounterChannel {
  fn derive_channel(self, _state: &CounterState) -> Vec<Self> {
    vec![Self::Count, Self::Persistence]
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PersistenceStatus {
  Loaded,
  Saved,
  Failed(String),
}

impl DataReducer for CounterState {
  type Action = CounterEvent;
  type Channel = CounterChannel;

  fn reduce(&mut self, event: Self::Action) -> ChannelSelection<Self::Channel> {
    match event {
      CounterEvent::Increment => IncrementCounter.execute(&mut self.counter),
      CounterEvent::Decrement => DecrementCounter.execute(&mut self.counter),
      CounterEvent::SaveNow => {}
    }
    self.persist();

    ChannelSelection::Select(CounterChannel::Count)
  }
}

#[cfg(test)]
mod tests {
  use std::cell::RefCell;

  use super::*;
  use crate::domain::{entities::CounterId, repositories::CounterRepositoryError};

  struct RecordingRepository {
    loaded: Option<Counter>,
    saved_values: RefCell<Vec<i32>>,
    fail_saves: bool,
  }

  impl RecordingRepository {
    fn empty() -> Self {
      Self {
        loaded: None,
        saved_values: RefCell::new(Vec::new()),
        fail_saves: false,
      }
    }
  }

  impl CounterRepository for RecordingRepository {
    fn load(&self) -> Result<Option<Counter>, CounterRepositoryError> {
      Ok(self.loaded)
    }

    fn save(&self, counter: &Counter) -> Result<(), CounterRepositoryError> {
      if self.fail_saves {
        return Err(CounterRepositoryError::new("disk unavailable"));
      }

      self.saved_values.borrow_mut().push(counter.value());
      Ok(())
    }
  }

  fn state_with(repository: Rc<RecordingRepository>) -> CounterState {
    CounterState::restore(
      Counter::new(CounterId::new(1), 4),
      repository,
      "test-counter.json".to_string(),
    )
  }

  #[test]
  fn increment_event_runs_the_use_case_and_persists_the_value() {
    let repository = Rc::new(RecordingRepository::empty());
    let mut state = state_with(Rc::clone(&repository));

    state.reduce(CounterEvent::Increment);

    assert_eq!(state.count(), 5);
    assert_eq!(*repository.saved_values.borrow(), vec![4, 5]);
  }

  #[test]
  fn restore_uses_the_value_loaded_from_the_repository() {
    let repository = Rc::new(RecordingRepository {
      loaded: Some(Counter::new(CounterId::new(1), 18)),
      saved_values: RefCell::new(Vec::new()),
      fail_saves: false,
    });

    let state = state_with(repository);

    assert_eq!(state.count(), 18);
    assert_eq!(state.persistence_status(), &PersistenceStatus::Loaded);
  }

  #[test]
  fn save_failure_is_exposed_in_the_state() {
    let repository = Rc::new(RecordingRepository {
      loaded: Some(Counter::new(CounterId::new(1), 4)),
      saved_values: RefCell::new(Vec::new()),
      fail_saves: true,
    });
    let mut state = state_with(repository);

    state.reduce(CounterEvent::SaveNow);

    assert_eq!(
      state.persistence_status(),
      &PersistenceStatus::Failed("disk unavailable".to_string())
    );
  }
}
