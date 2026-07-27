#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct CounterId(u64);

impl CounterId {
  pub(crate) const fn new(value: u64) -> Self {
    Self(value)
  }

  pub(crate) const fn value(self) -> u64 {
    self.0
  }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Counter {
  id: CounterId,
  value: i32,
}

impl Counter {
  pub(crate) const fn new(id: CounterId, value: i32) -> Self {
    Self { id, value }
  }

  pub(crate) const fn id(self) -> CounterId {
    self.id
  }

  pub(crate) const fn value(self) -> i32 {
    self.value
  }

  pub(crate) const fn increment(&mut self) {
    self.value = self.value.saturating_add(1);
  }

  pub(crate) const fn decrement(&mut self) {
    self.value = self.value.saturating_sub(1);
  }
}

impl PartialEq for Counter {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
  }
}

impl Eq for Counter {}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn counters_with_the_same_id_have_the_same_identity() {
    let first = Counter::new(CounterId::new(1), 4);
    let second = Counter::new(CounterId::new(1), 99);

    assert_eq!(first, second);
  }

  #[test]
  fn counter_value_saturates_at_integer_bounds() {
    let mut maximum = Counter::new(CounterId::new(1), i32::MAX);
    let mut minimum = Counter::new(CounterId::new(2), i32::MIN);

    maximum.increment();
    minimum.decrement();

    assert_eq!(maximum.value(), i32::MAX);
    assert_eq!(minimum.value(), i32::MIN);
  }
}
