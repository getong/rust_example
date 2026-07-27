use crate::domain::{
  entities::Counter,
  repositories::{CounterRepository, CounterRepositoryError},
};

pub(crate) struct LoadCounter<'repository, Repository: ?Sized> {
  repository: &'repository Repository,
}

impl<'repository, Repository> LoadCounter<'repository, Repository>
where
  Repository: CounterRepository + ?Sized,
{
  pub(crate) const fn new(repository: &'repository Repository) -> Self {
    Self { repository }
  }

  pub(crate) fn execute(&self) -> Result<Option<Counter>, CounterRepositoryError> {
    self.repository.load()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::domain::entities::CounterId;

  struct StubCounterRepository {
    counter: Counter,
  }

  impl CounterRepository for StubCounterRepository {
    fn load(&self) -> Result<Option<Counter>, CounterRepositoryError> {
      Ok(Some(self.counter))
    }

    fn save(&self, _counter: &Counter) -> Result<(), CounterRepositoryError> {
      Ok(())
    }
  }

  #[test]
  fn execute_loads_the_counter_from_the_repository() {
    let repository = StubCounterRepository {
      counter: Counter::new(CounterId::new(7), 12),
    };

    let counter = LoadCounter::new(&repository)
      .execute()
      .expect("stub repository should load successfully")
      .expect("stub repository should contain a counter");

    assert_eq!(counter, Counter::new(CounterId::new(7), 0));
    assert_eq!(counter.value(), 12);
  }
}
