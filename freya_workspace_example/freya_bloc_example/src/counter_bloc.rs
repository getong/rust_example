use freya::radio::{ChannelSelection, DataReducer, RadioChannel};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CounterState {
  count: i32,
}

impl CounterState {
  pub(crate) const fn new(count: i32) -> Self {
    Self { count }
  }

  pub(crate) const fn count(self) -> i32 {
    self.count
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CounterEvent {
  Increment,
  Decrement,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum CounterChannel {
  Count,
}

impl RadioChannel<CounterState> for CounterChannel {}

impl DataReducer for CounterState {
  type Action = CounterEvent;
  type Channel = CounterChannel;

  fn reduce(&mut self, event: Self::Action) -> ChannelSelection<Self::Channel> {
    self.count = match event {
      CounterEvent::Increment => self.count.saturating_add(1),
      CounterEvent::Decrement => self.count.saturating_sub(1),
    };

    ChannelSelection::Current
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn increment_event_increases_count() {
    let mut state = CounterState::new(4);

    state.reduce(CounterEvent::Increment);

    assert_eq!(state.count(), 5);
  }

  #[test]
  fn decrement_event_decreases_count() {
    let mut state = CounterState::new(4);

    state.reduce(CounterEvent::Decrement);

    assert_eq!(state.count(), 3);
  }

  #[test]
  fn counter_does_not_overflow() {
    let mut maximum = CounterState::new(i32::MAX);
    let mut minimum = CounterState::new(i32::MIN);

    maximum.reduce(CounterEvent::Increment);
    minimum.reduce(CounterEvent::Decrement);

    assert_eq!(maximum.count(), i32::MAX);
    assert_eq!(minimum.count(), i32::MIN);
  }
}
