use crate::domain::{entities::Counter, repositories::CounterRepository};

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

  pub(crate) fn execute(&self) -> Counter {
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
    fn load(&self) -> Counter {
      self.counter
    }
  }

  #[test]
  fn execute_loads_the_counter_from_the_repository() {
    let repository = StubCounterRepository {
      counter: Counter::new(CounterId::new(7), 12),
    };

    let counter = LoadCounter::new(&repository).execute();

    assert_eq!(counter, Counter::new(CounterId::new(7), 0));
    assert_eq!(counter.value(), 12);
  }
}
