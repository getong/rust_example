#![cfg(target_arch = "wasm32")]

wit_bindgen::generate!({
  path: "src/wit",
  world: "the-world",
});

use crate::a::b::temperature_types;

struct TemperatureDemo;

impl exports::temperature_service::Guest for TemperatureDemo {
  fn calculate_celsius(
    mut state: temperature_types::HostState,
  ) -> temperature_types::TemperatureResult {
    state.temperature_read_count += 1;

    let fahrenheit = state.current_fahrenheit;
    state.conversion_count += 1;
    state.last_fahrenheit = Some(fahrenheit);

    let celsius = (fahrenheit - 32.0) * 5.0 / 9.0;
    state.last_celsius = Some(celsius);

    temperature_types::TemperatureResult {
      celsius: temperature_types::Celsius { degrees: celsius },
      state,
    }
  }
}

export!(TemperatureDemo);
