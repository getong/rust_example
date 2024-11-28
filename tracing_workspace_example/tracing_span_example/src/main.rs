use std::fmt::Debug;

use reqwest::Client;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tracing::{trace, Instrument};

#[derive(Debug)]
pub struct HttpClient {
  client: Client,
}

#[derive(Debug)]
pub enum HttpClientError {
  RequestFailed(reqwest::Error),
  SerializationError(serde_json::Error),
}

impl From<reqwest::Error> for HttpClientError {
  fn from(err: reqwest::Error) -> Self {
    HttpClientError::RequestFailed(err)
  }
}

impl From<serde_json::Error> for HttpClientError {
  fn from(err: serde_json::Error) -> Self {
    HttpClientError::SerializationError(err)
  }
}

impl HttpClient {
  pub fn new() -> Self {
    Self {
      client: Client::new(),
    }
  }

  pub async fn send_request<T, R>(
    &self,
    method: &str,
    url: &str,
    params: T,
  ) -> Result<R, HttpClientError>
  where
    T: Debug + Serialize + Send + Sync, // T must be serializable
    R: DeserializeOwned + Debug + Send, // R must be deserializable
  {
    // Start tracing span
    let span = tracing::trace_span!("http_request", method = method, url = url, params = ?serde_json::to_string(&params).unwrap_or_default());

    let result = async move {
      trace!("Sending request");

      // Perform the HTTP request
      let res = self
        .client
        .post(url)
        .json(&params) // Send the params as JSON
        .send()
        .await?
        .json::<R>() // Expect a response of type R
        .await?;

      // Log the response using Debug instead of serializing it to JSON
      trace!(response = ?res, "Received response");

      Ok::<_, HttpClientError>(res)
    }
    .instrument(span) // Instrumenting with tracing span
    .await;

    result
  }
}

#[derive(Debug, Serialize, Deserialize)] // Ensure the derive macro is available
struct ApiRequest {
  query: String,
}

#[derive(Debug, Deserialize)] // Ensure the derive macro is available
pub struct ApiResponse {
  pub result: String,
}

#[tokio::main]
async fn main() -> Result<(), HttpClientError> {
  // Initialize tracing
  tracing_subscriber::fmt::init();

  let client = HttpClient::new();

  let request_data = ApiRequest {
    query: "example query".to_string(),
  };

  // Make a request
  let response: ApiResponse = client
    .send_request("POST", "https://api.example.com/endpoint", request_data)
    .await?;

  println!("API Response: {:?}", response);
  Ok(())
}
