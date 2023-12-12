use std::{error::Error, thread::sleep, time::Duration};
use zbus::{dbus_interface, ConnectionBuilder};

struct Greeter {
  count: u64,
}

#[dbus_interface(name = "org.zbus.MyGreeter1")]
impl Greeter {
  // Can be `async` as well.
  fn say_hello(&mut self, name: &str) -> String {
    self.count += 1;
    format!("Hello {}! I have been called: {}", name, self.count)
  }
}

// Although we use `async-std` here, you can use any async runtime of choice.
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  let greeter = Greeter { count: 0 };
  let _ = ConnectionBuilder::session()?
    .name("org.zbus.MyGreeter")?
    .serve_at("/tmp/MyGreeter", greeter)?
    .build()
    .await?;

  // Do other things or go to sleep.
  sleep(Duration::from_secs(60));

  Ok(())
}
