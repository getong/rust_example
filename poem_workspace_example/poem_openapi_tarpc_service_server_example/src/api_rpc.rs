use tarpc::{client, context, tokio_serde::formats::Json};

use crate::common::Api;

#[tarpc::service]
pub trait World {
  /// Returns a greeting for name.
  async fn hello(name: String) -> String;
}

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

pub async fn send_hello_msg() -> Result<String, std::io::Error> {
  let mut transport = tarpc::serde_transport::tcp::connect("127.0.0.1:3000", Json::default);
  transport.config_mut().max_frame_length(usize::MAX);
  let client = WorldClient::new(client::Config::default(), transport.await?).spawn();
  match client
    .hello(context::current(), format!("{}1", "hello"))
    .await
  {
    Ok(result) => Ok(result),
    Err(e) => {
      // Manually handle the conversion from RpcError to std::io::Error
      let io_error = std::io::Error::new(std::io::ErrorKind::Other, e.to_string());
      Err(io_error)
    }
  }
}
