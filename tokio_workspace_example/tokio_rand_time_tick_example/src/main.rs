use std::time::Duration;

use chrono::Utc;
use rand::distributions::{Distribution, Uniform};
use tokio::time;

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
  let (send, recv) = tokio::sync::mpsc::channel(100);

  let shutdown_trigger = tokio::spawn(async move {
    let mut i = 1;
    tokio::time::sleep(Duration::from_secs(1)).await;

    // We get the expected behavior by commenting out this line.
    i += 1;
    send.send(i).await.unwrap();
  });

  let done = run_with_shutdown(recv);
  done.await;
  shutdown_trigger.await.unwrap();

  Ok(())
}

async fn run_with_shutdown(mut shutdown: tokio::sync::mpsc::Receiver<i32>) {
  let mut interval = time::interval(time::Duration::from_secs(1));
  let between = Uniform::from(1 .. 4);
  let mut rng = rand::thread_rng();

  loop {
    tokio::select! {
        _ = interval.tick() => println!("tick, now: {:?}", Utc::now()),
        number = shutdown.recv() =>  {
            if let Some(i) = number {
                println!("shutting down, number is {:?}, now: {:?}", i, Utc::now());
                let rand_interval_time = between.sample(&mut rng);
                println!("rand_interval_time:{}", rand_interval_time);
                interval = time::interval(time::Duration::from_secs(rand_interval_time));
                // return
            }
        },
    }
  }
}
