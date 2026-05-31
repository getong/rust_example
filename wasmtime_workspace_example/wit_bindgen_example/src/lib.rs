#![cfg(target_arch = "wasm32")]

wit_bindgen::generate!({
  path: "src/wit",
  world: "the-world",
});

struct TemperatureDemo;

impl exports::temperature_service::Guest for TemperatureDemo {
  fn run() -> exports::temperature_service::Celsius {
    let current_temp = thermometer::what_temperature_is_it();
    let in_celsius = thermometer::convert_to_celsius(current_temp);

    exports::temperature_service::Celsius {
      degrees: in_celsius.degrees,
    }
  }
}

export!(TemperatureDemo);
