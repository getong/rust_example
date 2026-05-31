#![cfg(target_arch = "wasm32")]

wit_bindgen::generate!({
  path: "src/wit",
  world: "the-world",
});

use crate::a::b::temperature_types;

struct TemperatureDemo;

impl exports::temperature_service::Guest for TemperatureDemo {
  fn calculate_celsius() -> temperature_types::Celsius {
    let current_temp = thermometer::what_temperature_is_it();
    let in_celsius = thermometer::convert_to_celsius(current_temp);

    temperature_types::Celsius {
      degrees: in_celsius.degrees,
    }
  }
}

export!(TemperatureDemo);
