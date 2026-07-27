use crate::domain::{entities::Counter, repositories::CounterRepository};

pub(crate) struct InMemoryCounterRepository {
  counter: Counter,
}

impl InMemoryCounterRepository {
  pub(crate) const fn new(counter: Counter) -> Self {
    Self { counter }
  }
}

impl CounterRepository for InMemoryCounterRepository {
  fn load(&self) -> Counter {
    self.counter
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::domain::entities::CounterId;

  #[test]
  fn load_returns_the_stored_counter() {
    let repository = InMemoryCounterRepository::new(Counter::new(CounterId::new(3), 8));

    let counter = repository.load();

    assert_eq!(counter, Counter::new(CounterId::new(3), 0));
    assert_eq!(counter.value(), 8);
  }
}
