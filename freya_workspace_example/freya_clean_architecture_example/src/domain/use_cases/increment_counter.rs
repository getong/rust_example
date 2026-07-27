use crate::domain::entities::Counter;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct IncrementCounter;

impl IncrementCounter {
  pub(crate) const fn execute(self, counter: &mut Counter) {
    counter.increment();
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::domain::entities::CounterId;

  #[test]
  fn execute_increases_the_counter_value() {
    let mut counter = Counter::new(CounterId::new(1), 4);

    IncrementCounter.execute(&mut counter);

    assert_eq!(counter.value(), 5);
  }
}
