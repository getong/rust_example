use tarpc::context;

use crate::common::Api;

#[tarpc::service]
pub trait World {
  /// Returns a greeting for name.
  async fn hello(name: String) -> String;
}

#[tarpc::server]
impl World for Api {
  async fn hello(self, _context_info: context::Context, name: String) -> String {
    let mut num = self.num.lock().await;
    *num += 1;
    format!(
      "Hello, {name}! You are connected from {}, access num is {}",
      name, num
    )
  }
}
