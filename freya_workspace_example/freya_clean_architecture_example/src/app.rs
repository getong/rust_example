use std::{env, path::PathBuf, rc::Rc};

use freya::{
  prelude::IntoElement,
  radio::use_init_radio_station,
  router::{Router, RouterConfig},
};

use crate::{
  data::repositories::JsonFileCounterRepository,
  domain::{
    entities::{Counter, CounterId},
    repositories::CounterRepository,
  },
  presentation::{
    counter::{CounterChannel, CounterState},
    navigation::Route,
  },
};

const INITIAL_COUNT: i32 = 4;
const MAIN_COUNTER_ID: CounterId = CounterId::new(1);

/// Builds the Freya component tree and wires its clean-architecture dependencies.
pub fn app() -> impl IntoElement {
  use_init_radio_station::<CounterState, CounterChannel>(|| {
    let storage_path = counter_storage_path();
    let storage_location = storage_path.display().to_string();
    let repository: Rc<dyn CounterRepository> =
      Rc::new(JsonFileCounterRepository::new(storage_path));
    let default_counter = Counter::new(MAIN_COUNTER_ID, INITIAL_COUNT);

    CounterState::restore(default_counter, repository, storage_location)
  });

  Router::<Route>::new(|| RouterConfig::default().with_initial_path(Route::Counter))
}

fn counter_storage_path() -> PathBuf {
  env::var_os("HOME")
    .or_else(|| env::var_os("USERPROFILE"))
    .map_or_else(env::temp_dir, PathBuf::from)
    .join(".freya_clean_architecture_example")
    .join("counter.json")
}
