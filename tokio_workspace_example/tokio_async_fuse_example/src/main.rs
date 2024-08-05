use async_fuse::Fuse;
use std::time::Duration;
use tokio::time;

#[tokio::main]
async fn main() {
  let mut duration = Duration::from_millis(500);

  let sleep = Fuse::new(time::sleep(duration));
  let mut sleep = std::pin::pin!(sleep);

  let update_duration = Fuse::new(time::sleep(Duration::from_secs(1)));
  let mut update_duration = std::pin::pin!(update_duration);

  for _ in 0 .. 10usize {
    tokio::select! {
        _ = &mut sleep => {
            println!("Tick");
            sleep.set(Fuse::new(time::sleep(duration)));
        }
        _ = &mut update_duration => {
            println!("Tick faster!");
            duration = Duration::from_millis(250);
        }
    }
  }
}
