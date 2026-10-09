use std::time::Duration;

use serde::de::DeserializeOwned;
use topcoat_gpui_protocol::crypto::{
  self, ApiRequest, ApiResponse, ClientHandshake, Envelope, ServerHello,
};

/// Per-handshake ephemeral keys. Remote server authentication relies on HTTPS.
pub struct SecureClient {
  http: reqwest::Client,
  base: String,
}
impl SecureClient {
  pub fn new(base: &str) -> Result<Self, String> {
    let url = reqwest::Url::parse(base).map_err(|e| e.to_string())?;
    if !matches!(url.scheme(), "http" | "https")
      || url.host_str().is_none()
      || !url.username().is_empty()
      || url.password().is_some()
      || url.query().is_some()
      || url.fragment().is_some()
      || url.path() != "/"
    {
      return Err(
        "TOPCOAT_URL must be an http(s) origin without credentials, path or query".into(),
      );
    }
    if url.scheme() == "http"
      && !matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"))
    {
      return Err(
        "Dynamic public keys require HTTPS for remote servers; HTTP is only allowed on localhost"
          .into(),
      );
    }
    let http = reqwest::Client::builder()
      .connect_timeout(Duration::from_secs(2))
      .timeout(Duration::from_secs(10))
      .redirect(reqwest::redirect::Policy::none())
      .build()
      .map_err(|e| e.to_string())?;
    Ok(Self {
      http,
      base: base.trim_end_matches('/').into(),
    })
  }
  async fn post<T: serde::Serialize, R: DeserializeOwned>(
    &self,
    path: &str,
    body: &T,
    limit: usize,
  ) -> Result<R, String> {
    let mut response = self
      .http
      .post(format!("{}{path}", self.base))
      .json(body)
      .send()
      .await
      .map_err(|e| e.to_string())?
      .error_for_status()
      .map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
      if chunk.len() > limit.saturating_sub(bytes.len()) {
        return Err("Encrypted response exceeds size limit".into());
      }
      bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
  }
  pub async fn request<T: DeserializeOwned>(&self, request: ApiRequest) -> Result<T, String> {
    let result = async {
      let (handshake, hello) = ClientHandshake::begin().map_err(|e| e.to_string())?;
      let reply: ServerHello = self.post(crypto::HANDSHAKE_PATH, &hello, 20000).await?;
      let channel = handshake
        .finish(&reply, &reply.public_key)
        .map_err(|e| e.to_string())?;
      let body = serde_json::to_vec(&request).map_err(|e| e.to_string())?;
      let (envelope, reader) = channel.seal(&body).map_err(|e| e.to_string())?;
      let response: Envelope = self
        .post(crypto::EXCHANGE_PATH, &envelope, crypto::MAX_WIRE)
        .await?;
      let plaintext = reader.open(&response).map_err(|e| e.to_string())?;
      let reply: ApiResponse = serde_json::from_slice(&plaintext).map_err(|e| e.to_string())?;
      if !(200 .. 300).contains(&reply.status) {
        return Err(format!("HTTP {}: {}", reply.status, reply.body));
      }
      serde_json::from_str(&reply.body).map_err(|e| e.to_string())
    }
    .await;
    result.map_err(|e| format!("{e}. Refresh to check server state before retrying a change."))
  }
}
