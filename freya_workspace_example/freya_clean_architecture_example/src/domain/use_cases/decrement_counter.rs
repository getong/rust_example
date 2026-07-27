use crate::domain::entities::Counter;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct DecrementCounter;

impl DecrementCounter {
  pub(crate) const fn execute(self, counter: &mut Counter) {
    counter.decrement();
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::domain::entities::CounterId;

  #[test]
  fn execute_decreases_the_counter_value() {
    let mut counter = Counter::new(CounterId::new(1), 4);

    DecrementCounter.execute(&mut counter);

    assert_eq!(counter.value(), 3);
  }
}
