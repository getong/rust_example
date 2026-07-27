use crate::domain::{
  entities::Counter,
  repositories::{CounterRepository, CounterRepositoryError},
};

pub(crate) struct SaveCounter<'repository, Repository: ?Sized> {
  repository: &'repository Repository,
}

impl<'repository, Repository> SaveCounter<'repository, Repository>
where
  Repository: CounterRepository + ?Sized,
{
  pub(crate) const fn new(repository: &'repository Repository) -> Self {
    Self { repository }
  }

  pub(crate) fn execute(&self, counter: &Counter) -> Result<(), CounterRepositoryError> {
    self.repository.save(counter)
  }
}

#[cfg(test)]
mod tests {
  use std::cell::RefCell;

  use super::*;
  use crate::domain::entities::CounterId;

  #[derive(Default)]
  struct RecordingCounterRepository {
    saved_value: RefCell<Option<i32>>,
  }

  impl CounterRepository for RecordingCounterRepository {
    fn load(&self) -> Result<Option<Counter>, CounterRepositoryError> {
      Ok(None)
    }

    fn save(&self, counter: &Counter) -> Result<(), CounterRepositoryError> {
      self.saved_value.replace(Some(counter.value()));
      Ok(())
    }
  }

  #[test]
  fn execute_saves_the_counter_through_the_repository() {
    let repository = RecordingCounterRepository::default();
    let counter = Counter::new(CounterId::new(1), 12);

    SaveCounter::new(&repository)
      .execute(&counter)
      .expect("recording repository should save successfully");

    assert_eq!(*repository.saved_value.borrow(), Some(12));
  }
}
