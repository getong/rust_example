use freya::{prelude::IntoElement, radio::use_init_radio_station};

use crate::{
  data::repositories::InMemoryCounterRepository,
  domain::{
    entities::{Counter, CounterId},
    use_cases::LoadCounter,
  },
  presentation::counter::{CounterChannel, CounterState, CounterView},
};

const INITIAL_COUNT: i32 = 4;
const MAIN_COUNTER_ID: CounterId = CounterId::new(1);

/// Builds the Freya component tree and wires its clean-architecture dependencies.
pub fn app() -> impl IntoElement {
  use_init_radio_station::<CounterState, CounterChannel>(|| {
    let counter = Counter::new(MAIN_COUNTER_ID, INITIAL_COUNT);
    let repository = InMemoryCounterRepository::new(counter);
    let load_counter = LoadCounter::new(&repository);

    CounterState::new(load_counter.execute())
  });

  CounterView
}
