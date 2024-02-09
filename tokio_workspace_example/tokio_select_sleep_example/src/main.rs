use tokio::time::{self, Duration, Instant};

#[tokio::main]
async fn main() {
  let sleep = time::sleep(Duration::from_millis(10));
  let mut sleep = std::pin::pin!(sleep);

  loop {
    tokio::select! {
        () = &mut sleep => {
            println!("timer elapsed");
            sleep.as_mut().reset(Instant::now() + Duration::from_millis(50));
        },
    }
  }
}
