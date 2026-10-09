//! Encrypted HTTP client shared by the desktop and integration smoke commands.
#[cfg(feature = "desktop")]
pub mod runtime;
pub mod secure;
use secure::SecureClient;
use topcoat_gpui_protocol::{crypto::ApiRequest, *};

fn api_request(path: &str, body: Option<String>, form: bool) -> ApiRequest {
  ApiRequest {
    method: if body.is_some() { "POST" } else { "GET" }.into(),
    path: path.into(),
    body: body.unwrap_or_default(),
    form,
  }
}
pub async fn request(
  base_url: &str,
  action: Option<CounterAction>,
) -> Result<CounterSnapshot, String> {
  let body = action
    .map(|action| serde_json::to_string(&UpdateCounter { action }))
    .transpose()
    .map_err(|e| e.to_string())?;
  SecureClient::new(base_url)?
    .request(api_request(COUNTER_PATH, body, false))
    .await
}
#[derive(Clone, Debug)]
pub enum DemoCommand {
  Refresh,
  Todo(TodoCommand),
  Echo(serde_json::Value),
  Profile(Profile),
}
pub async fn demo_request(base_url: &str, command: DemoCommand) -> Result<DemoSnapshot, String> {
  let (path, body) = match command {
    DemoCommand::Refresh => (DEMOS_PATH, None),
    DemoCommand::Todo(value) => (
      TODOS_PATH,
      Some(serde_json::to_value(value).map_err(|e| e.to_string())?),
    ),
    DemoCommand::Echo(value) => (ECHO_PATH, Some(value)),
    DemoCommand::Profile(value) => (
      PROFILE_PATH,
      Some(serde_json::to_value(value).map_err(|e| e.to_string())?),
    ),
  };
  SecureClient::new(base_url)?
    .request(api_request(path, body.map(|v| v.to_string()), false))
    .await
}
pub async fn studio_request(
  base_url: &str,
  command: Option<StudioCommand>,
) -> Result<StudioSnapshot, String> {
  let body = command
    .map(|c| serde_json::to_string(&c))
    .transpose()
    .map_err(|e| e.to_string())?;
  SecureClient::new(base_url)?
    .request(api_request(STUDIO_PATH, body, false))
    .await
}
