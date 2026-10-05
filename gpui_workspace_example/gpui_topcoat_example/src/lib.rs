//! HTTP client shared by the desktop view and the integration smoke command.
use std::time::Duration;

use topcoat_gpui_protocol::{COUNTER_PATH, CounterAction, CounterSnapshot, UpdateCounter};

/// Call on a background thread, never on the GPUI UI thread.
/// Mutations are intentionally not retried: a lost response can follow a committed write.
pub fn request(base_url: &str, action: Option<CounterAction>) -> Result<CounterSnapshot, String> {
  let client = reqwest::blocking::Client::builder()
    .connect_timeout(Duration::from_secs(2))
    .timeout(Duration::from_secs(5))
    .redirect(reqwest::redirect::Policy::none())
    .build()
    .map_err(|e| e.to_string())?;
  let url = format!("{}{COUNTER_PATH}", base_url.trim_end_matches('/'));
  let request = match action {
    None => client.get(url),
    Some(action) => client.post(url).json(&UpdateCounter { action }),
  };
  request
    .send()
    .and_then(reqwest::blocking::Response::error_for_status)
    .and_then(|response| response.json())
    .map_err(|e| format!("{e}. Refresh to check the server state before retrying a change."))
}

use topcoat_gpui_protocol::{
  DEMOS_PATH, DemoSnapshot, ECHO_PATH, PROFILE_PATH, Profile, TODOS_PATH, TodoCommand,
};

#[derive(Clone, Debug)]
pub enum DemoCommand {
  Refresh,
  Todo(TodoCommand),
  Echo(serde_json::Value),
  Profile(Profile),
}

/// Each mutation returns the same complete snapshot as GET, avoiding a second fetch.
pub fn demo_request(base_url: &str, command: DemoCommand) -> Result<DemoSnapshot, String> {
  let client = reqwest::blocking::Client::builder()
    .connect_timeout(Duration::from_secs(2))
    .timeout(Duration::from_secs(5))
    .redirect(reqwest::redirect::Policy::none())
    .build()
    .map_err(|e| e.to_string())?;
  let base = base_url.trim_end_matches('/');
  let request = match command {
    DemoCommand::Refresh => client.get(format!("{base}{DEMOS_PATH}")),
    DemoCommand::Todo(command) => client.post(format!("{base}{TODOS_PATH}")).json(&command),
    DemoCommand::Echo(value) => client.post(format!("{base}{ECHO_PATH}")).json(&value),
    DemoCommand::Profile(profile) => client.post(format!("{base}{PROFILE_PATH}")).form(&profile),
  };
  let response = request
    .send()
    .map_err(|e| format!("{e}. Refresh before retrying a change."))?;
  let status = response.status();
  if !status.is_success() {
    return Err(format!(
      "HTTP {status}: {}",
      response.text().unwrap_or_default()
    ));
  }
  response.json().map_err(|e| e.to_string())
}

/// Independent palette endpoint used by the native studio and its smoke test.
pub fn studio_request(
  base_url: &str,
  command: Option<topcoat_gpui_protocol::StudioCommand>,
) -> Result<topcoat_gpui_protocol::StudioSnapshot, String> {
  let client = reqwest::blocking::Client::builder()
    .connect_timeout(Duration::from_secs(2))
    .timeout(Duration::from_secs(5))
    .redirect(reqwest::redirect::Policy::none())
    .build()
    .map_err(|e| e.to_string())?;
  let url = format!(
    "{}{}",
    base_url.trim_end_matches('/'),
    topcoat_gpui_protocol::STUDIO_PATH
  );
  let request = match command {
    Some(command) => client.post(url).json(&command),
    None => client.get(url),
  };
  let response = request.send().map_err(|e| e.to_string())?;
  if !response.status().is_success() {
    return Err(format!(
      "HTTP {}: {}",
      response.status(),
      response.text().unwrap_or_default()
    ));
  }
  response.json().map_err(|e| e.to_string())
}
