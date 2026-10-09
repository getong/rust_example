//! Direct calls to endpoints owned by Axum, outside the Topcoat encrypted gateway.
use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct Health {
  pub status: String,
}

/// Liveness only: this does not establish business synchronization or authentication.
pub async fn health(base_url: &str) -> Result<Health, String> {
  let (http, base) = crate::secure::transport(base_url)?;
  let mut response = http
    .get(format!("{base}/healthz"))
    .send()
    .await
    .map_err(|e| format!("Health request failed: {e}"))?;
  if response.status() != reqwest::StatusCode::OK {
    return Err(format!("Health HTTP {}", response.status()));
  }
  let mut bytes = Vec::new();
  while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
    if chunk.len() > 1024usize.saturating_sub(bytes.len()) {
      return Err("Health response exceeds size limit".into());
    }
    bytes.extend_from_slice(&chunk);
  }
  let health: Health =
    serde_json::from_slice(&bytes).map_err(|e| format!("Invalid health response: {e}"))?;
  if health.status != "ok" {
    return Err(format!("Service is not healthy: {}", health.status));
  }
  Ok(health)
}
