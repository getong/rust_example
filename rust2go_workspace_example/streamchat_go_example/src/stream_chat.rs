pub mod binding {
  #![allow(warnings)]
  rust2go::r2g_include_binding!();
}

#[derive(rust2go::R2G, Clone)]
pub struct StreamChatTokenRequest {
  pub api_key: String,
  pub api_secret: String,
  pub user_id: String,
}

#[derive(rust2go::R2G, Clone)]
pub struct StreamChatTokenResponse {
  pub token: String,
  pub error: String,
}

#[rust2go::r2g]
pub trait StreamChatCall {
  fn create_token(req: &StreamChatTokenRequest) -> StreamChatTokenResponse;
}
