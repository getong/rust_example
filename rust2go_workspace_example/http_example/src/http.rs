pub mod binding {
  #![allow(warnings)]
  rust2go::r2g_include_binding!();
}

#[derive(rust2go::R2G, Clone)]
pub struct HttpFetchRequest {
  pub url: String,
  pub max_lines: u32,
}

#[derive(rust2go::R2G, Clone)]
pub struct HttpFetchResponse {
  pub status: String,
  pub lines: Vec<String>,
  pub error: String,
}

#[rust2go::r2g]
pub trait HttpFetchCall {
  fn fetch(req: &HttpFetchRequest) -> HttpFetchResponse;
}
