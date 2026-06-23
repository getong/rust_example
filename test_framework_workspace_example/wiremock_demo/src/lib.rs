//! Demonstrates HTTP client logic tested with `wiremock`.

use serde::Deserialize;

/// User data returned by a remote JSON API.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct User {
  /// Stable user id.
  pub id: u64,
  /// Display name.
  pub name: String,
}

/// Builds the API path used to fetch a user.
#[must_use]
pub fn user_path(user_id: u64) -> String {
  format!("/users/{user_id}")
}

/// Parses a user JSON response body.
///
/// # Errors
///
/// Returns a serde error when `body` is not valid user JSON.
pub fn parse_user(body: &str) -> Result<User, serde_json::Error> {
  serde_json::from_str(body)
}

#[cfg(test)]
mod tests {
  use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
  };

  use super::*;

  #[tokio::test]
  async fn mock_server_matches_get_user_request() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
      .and(path(user_path(42)))
      .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
          "id": 42,
          "name": "Ada Lovelace"
      })))
      .mount(&server)
      .await;

    let response = reqwest::get(format!("{}{}", server.uri(), user_path(42)))
      .await
      .expect("mock server should respond");

    assert_eq!(response.status().as_u16(), 200);
    let body = response
      .text()
      .await
      .expect("mock response body should be readable");
    let user = parse_user(&body).expect("mock response body should be user json");

    assert_eq!(
      user,
      User {
        id: 42,
        name: String::from("Ada Lovelace"),
      }
    );
  }

  #[test]
  fn user_path_contains_id() {
    assert_eq!(user_path(9), "/users/9");
  }
}
