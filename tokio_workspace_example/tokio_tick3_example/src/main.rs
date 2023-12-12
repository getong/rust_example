use chrono::{DateTime, Local};
use rand::Rng;
use std::time::Duration;
use tokio::time;

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
  let (send, recv) = tokio::sync::mpsc::channel(100);

  let shutdown_trigger = tokio::spawn(async move {
    tokio::time::sleep(Duration::from_secs(15)).await;

    // We get the expected behavior by commenting out this line.
    send.send(()).await.unwrap();
  });

  let done = run_with_shutdown(recv);
  done.await;
  shutdown_trigger.await.unwrap();

  Ok(())
}

async fn run_with_shutdown(mut shutdown: tokio::sync::mpsc::Receiver<()>) {
  let mut interval = time::interval(Duration::from_secs(1));

  let mut rng = rand::thread_rng();

  loop {
    tokio::select! {
        _ = interval.tick() => {
            let current_time: DateTime<Local> = Local::now();
            // Format the date and time as a string
            let current_time_str = current_time.format("%Y-%m-%d %H:%M:%S").to_string();

            println!("tick, Current Time: {}", current_time_str);
            // Generate a random integer within a specific range
            let random_number = rng.gen_range(1..=4);
            interval = time::interval(Duration::from_secs(random_number));
            interval.reset();
        },
       _ = shutdown.recv() => {
            println!("shutting down");
            return
       },
    }
  }
}
