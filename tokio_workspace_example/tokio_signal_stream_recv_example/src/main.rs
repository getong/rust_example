use tokio::signal::unix::{signal, SignalKind};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  // An infinite stream of hangup signals.
  // let mut stream = signal(SignalKind::hangup())?;
  let mut stream = signal(SignalKind::terminate())?;

  // ps aux | grep recv
  // kill -15 $pid

  // Print whenever a HUP signal is received
  // loop {
  stream.recv().await;
  // println!("got signal HUP");
  println!("got signal SIGTERM");
  // }
  Ok(())
}
