use freya::radio::{ChannelSelection, DataReducer, RadioChannel};

use crate::domain::{
  entities::Counter,
  use_cases::{DecrementCounter, IncrementCounter},
};

#[derive(Clone, Copy, Debug)]
pub(crate) struct CounterState {
  counter: Counter,
}

impl CounterState {
  pub(crate) const fn new(counter: Counter) -> Self {
    Self { counter }
  }

  pub(crate) const fn count(self) -> i32 {
    self.counter.value()
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
    match event {
      CounterEvent::Increment => IncrementCounter.execute(&mut self.counter),
      CounterEvent::Decrement => DecrementCounter.execute(&mut self.counter),
    }

    ChannelSelection::Current
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::domain::entities::CounterId;

  #[test]
  fn increment_event_runs_the_increment_use_case() {
    let mut state = CounterState::new(Counter::new(CounterId::new(1), 4));

    state.reduce(CounterEvent::Increment);

    assert_eq!(state.count(), 5);
  }

  #[test]
  fn decrement_event_runs_the_decrement_use_case() {
    let mut state = CounterState::new(Counter::new(CounterId::new(1), 4));

    state.reduce(CounterEvent::Decrement);

    assert_eq!(state.count(), 3);
  }
}
