use std::fmt::Debug;

use reqwest::Client;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tokio::time::{sleep, Duration};
use tracing::{info, info_span, trace, Instrument};

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

  good_propagation().await;

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

async fn good_propagation() {
  async {
    sleep(Duration::from_millis(10)).await;
    info!("correctly attributed to my_task span");
  }
  .instrument(info_span!("my_task"))
  .await;
}

// async fn bad_propagation() {
//   let span = info_span!("my_task");
//   let _guard = span.enter();
//   sleep(Duration::from_millis(10)).await; // 守卫被跨让出点持有
//   info!("may log on a different thread with broken parent context");
// }
