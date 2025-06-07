use std::collections::HashMap;

use supabase_auth::models::{AuthClient, LoginWithOAuthOptions, LoginWithSSO, Provider};

#[tokio::main]
async fn main() {
  // You can manually pass in the values
  let auth_client = AuthClient::new("project_url", "api_key", "jwt_secret");

  // Or you can use environment variables
  // Requires `SUPABASE_URL`, `SUPABASE_API_KEY`, and `SUPABASE_JWT_SECRET` environment variables
  // let auth_client = AuthClient::new_from_env().unwrap();

  // Sign up methods return the session which you can use for creating cookies
  let _session = auth_client
    .sign_up_with_email_and_password("demo_email@gmail.com", "demo_password", None)
    .await
    .unwrap();

  // You can also sign up using a phone number and password
  let _session = auth_client
    .sign_up_with_phone_and_password("demo_phone", "demo_password", None)
    .await
    .unwrap();

  // Sign in methods return the session which you can use for creating cookies
  let _session = auth_client
    .login_with_email("demo_email", "demo_password")
    .await
    .unwrap();

  // You can also login using a phone number
  let _session = auth_client
    .login_with_phone("demo_phone", "demo_password")
    .await
    .unwrap();

  // Returns the provider and the url where the user will continue the auth flow
  let _oauth_response = auth_client
    .login_with_oauth(Provider::Github, None)
    .unwrap();

  // You can also customize the options like so:
  let mut query_params = HashMap::new();

  query_params.insert("key".to_string(), "value".to_string());
  query_params.insert("second_key".to_string(), "second_value".to_string());
  query_params.insert("third_key".to_string(), "third_value".to_string());

  let options = LoginWithOAuthOptions {
    query_params: Some(query_params),
    redirect_to: Some("your-redirect-url".to_string()),
    scopes: Some("repo gist notifications".to_string()),
    skip_brower_redirect: Some(true),
  };

  let _response = auth_client
    .login_with_oauth(Provider::Github, Some(options))
    .unwrap();

  let params = LoginWithSSO {
    domain: Some("demo_domain".to_string()),
    options: None,
    provider_id: None,
  };

  // Returns the URL where the user will continue the auth flow with your SSO provider
  let _url = auth_client.sso(params).await.unwrap();
}
