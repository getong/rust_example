#![cfg(target_arch = "wasm32")]

wit_bindgen::generate!({
  path: "src/wit",
  world: "the-world",
});

use serde::{Deserialize, Serialize};

use crate::a::b::temperature_types;

const TEMPERATURE_STATE_PATH: &str = "state/temperature.json";
const TEMPERATURE_STATE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TemperatureStatePayload {
  schema_version: u32,
  current_fahrenheit: f32,
  temperature_read_count: u32,
  conversion_count: u32,
  last_fahrenheit: Option<f32>,
  last_celsius: Option<f32>,
}

impl TemperatureStatePayload {
  fn empty() -> Self {
    Self {
      schema_version: TEMPERATURE_STATE_SCHEMA_VERSION,
      current_fahrenheit: 32.0,
      temperature_read_count: 0,
      conversion_count: 0,
      last_fahrenheit: None,
      last_celsius: None,
    }
  }
}

fn decode_state(state: &temperature_types::TemperatureState) -> TemperatureStatePayload {
  if state.path != TEMPERATURE_STATE_PATH {
    return TemperatureStatePayload::empty();
  }

  serde_json::from_str(&state.content_json)
    .ok()
    .filter(|payload: &TemperatureStatePayload| {
      payload.schema_version == TEMPERATURE_STATE_SCHEMA_VERSION
    })
    .unwrap_or_else(TemperatureStatePayload::empty)
}

fn encode_state(payload: &TemperatureStatePayload) -> String {
  serde_json::to_string(payload).unwrap_or_else(|_| "{}".to_string())
}

struct TemperatureDemo;

impl exports::temperature_service::Guest for TemperatureDemo {
  fn calculate_celsius(
    state: temperature_types::TemperatureState,
  ) -> temperature_types::TemperatureResult {
    let mut payload = decode_state(&state);
    payload.temperature_read_count += 1;

    let fahrenheit = payload.current_fahrenheit;
    payload.conversion_count += 1;
    payload.last_fahrenheit = Some(fahrenheit);

    let celsius = (fahrenheit - 32.0) * 5.0 / 9.0;
    payload.last_celsius = Some(celsius);

    temperature_types::TemperatureResult {
      celsius: temperature_types::Celsius { degrees: celsius },
      state: temperature_types::TemperatureState {
        path: TEMPERATURE_STATE_PATH.to_string(),
        content_json: encode_state(&payload),
      },
    }
  }
}

export!(TemperatureDemo);
