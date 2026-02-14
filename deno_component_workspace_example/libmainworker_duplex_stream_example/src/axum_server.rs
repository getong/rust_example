use std::{collections::HashMap, net::SocketAddr, path::PathBuf, process::Stdio};

use axum::{
  Json, Router,
  extract::State,
  http::StatusCode,
  routing::{get, post},
};
use deno_core::error::AnyError;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;

#[derive(Clone)]
struct AxumAppState {
  executable: PathBuf,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RunMainworkerRequest {
  target: Option<String>,
  args: Option<Vec<String>>,
  modules: Option<Vec<String>>,
  mfa: Option<Vec<String>>,
  env: Option<HashMap<String, String>>,
  messages: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RunMainworkerResponse {
  ok: bool,
  status_code: Option<i32>,
  target: String,
  result: Option<serde_json::Value>,
  exit_data: Option<serde_json::Value>,
  stdout_lines: Vec<String>,
  stderr: String,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
  ok: bool,
  error: String,
}

fn parse_json_or_string(raw: &str) -> serde_json::Value {
  serde_json::from_str(raw).unwrap_or_else(|_| serde_json::Value::String(raw.to_string()))
}

fn extract_marker_json(output: &str, marker: &str) -> Option<serde_json::Value> {
  output
    .lines()
    .filter_map(|line| line.strip_prefix(marker))
    .next_back()
    .map(parse_json_or_string)
}

fn api_error(
  status: StatusCode,
  message: impl Into<String>,
) -> (StatusCode, Json<ErrorResponse>) {
  (
    status,
    Json(ErrorResponse {
      ok: false,
      error: message.into(),
    }),
  )
}

async fn healthz() -> Json<serde_json::Value> {
  Json(serde_json::json!({ "ok": true }))
}

async fn run_mainworker(
  State(state): State<AxumAppState>,
  Json(payload): Json<RunMainworkerRequest>,
) -> Result<Json<RunMainworkerResponse>, (StatusCode, Json<ErrorResponse>)> {
  let target = payload
    .target
    .unwrap_or_else(|| "embed_deno/simple_main.ts".to_string());
  let runtime_args = payload.args.unwrap_or_default();
  let modules = payload.modules.unwrap_or_default();
  let mfa_values = payload.mfa.unwrap_or_default();
  let env = payload.env.unwrap_or_default();
  let messages = payload.messages.unwrap_or_default();
  let has_messages = !messages.is_empty();

  let mut command = tokio::process::Command::new(&state.executable);
  command.arg("--internal-run-once");
  command.arg("--oneshot");
  command.arg("--target");
  command.arg(&target);

  for module in &modules {
    command.arg("--module");
    command.arg(module);
  }
  for mfa in &mfa_values {
    command.arg("--mfa");
    command.arg(mfa);
  }
  if !runtime_args.is_empty() {
    command.arg("--");
    command.args(&runtime_args);
  }
  for (key, value) in env {
    command.env(key, value);
  }

  command.stdout(Stdio::piped());
  command.stderr(Stdio::piped());
  if has_messages {
    command.stdin(Stdio::piped());
  } else {
    command.stdin(Stdio::null());
  }

  let mut child = command
    .spawn()
    .map_err(|err| api_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;

  if has_messages {
    let Some(mut stdin) = child.stdin.take() else {
      return Err(api_error(
        StatusCode::INTERNAL_SERVER_ERROR,
        "failed to open child stdin",
      ));
    };
    for message in &messages {
      let line = serde_json::to_string(message)
        .map_err(|err| api_error(StatusCode::BAD_REQUEST, err.to_string()))?;
      stdin
        .write_all(line.as_bytes())
        .await
        .map_err(|err| api_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
      stdin
        .write_all(b"\n")
        .await
        .map_err(|err| api_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    }
  }

  let output = child
    .wait_with_output()
    .await
    .map_err(|err| api_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;

  let stdout = String::from_utf8_lossy(&output.stdout).to_string();
  let stderr = String::from_utf8_lossy(&output.stderr).to_string();
  let stdout_lines = stdout.lines().map(ToString::to_string).collect::<Vec<_>>();
  let result = extract_marker_json(&stdout, "EMBED_DENO_RESULT=");
  let exit_data = extract_marker_json(&stdout, "EMBED_DENO_EXIT_DATA=");

  let response = RunMainworkerResponse {
    ok: output.status.success(),
    status_code: output.status.code(),
    target,
    result,
    exit_data,
    stdout_lines,
    stderr,
  };

  if response.ok {
    Ok(Json(response))
  } else {
    Err(api_error(StatusCode::BAD_GATEWAY, response.stderr))
  }
}

pub async fn serve(addr: &str) -> Result<(), AnyError> {
  let socket_addr: SocketAddr = addr
    .parse()
    .map_err(|err| AnyError::msg(format!("invalid axum listen addr `{addr}`: {err}")))?;
  let executable = std::env::current_exe()
    .map_err(|err| AnyError::msg(format!("failed to resolve current executable: {err}")))?;
  let app_state = AxumAppState { executable };

  let app = Router::new()
    .route("/healthz", get(healthz))
    .route("/run", post(run_mainworker))
    .with_state(app_state);

  let listener = tokio::net::TcpListener::bind(socket_addr)
    .await
    .map_err(|err| AnyError::msg(format!("failed to bind {socket_addr}: {err}")))?;
  println!("axum mainworker api listening on http://{socket_addr}");
  axum::serve(listener, app)
    .await
    .map_err(|err| AnyError::msg(format!("axum server error: {err}")))?;
  Ok(())
}
