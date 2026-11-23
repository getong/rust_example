use std::{error::Error, time::Duration};

use tokio::time::sleep;
use zbus::{connection::Builder, interface};

struct Greeter {
  count: u64,
}

#[interface(name = "org.zbus.MyGreeter1")]
impl Greeter {
  // Can be `async` as well.
  fn say_hello(&mut self, name: &str) -> String {
    self.count += 1;
    format!("Hello {}! I have been called: {}", name, self.count)
  }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  let greeter = Greeter { count: 0 };
  let _connection = Builder::session()
    .name("org.zbus.MyGreeter")
    .serve_at("/tmp/MyGreeter", greeter)
    .build()
    .await?;

  // Do other things or go to sleep.
  sleep(Duration::from_secs(60)).await;

  Ok(())
}
