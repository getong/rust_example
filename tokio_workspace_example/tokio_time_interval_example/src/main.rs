use chrono::{DateTime, Local};
use tokio::time::{self, Duration};

#[tokio::main]
async fn main() {
  // Create a new interval that fires every 1 second
  let mut interval = time::interval(Duration::from_secs(1));

  // Run the interval for a total of 5 seconds
  for i in 1..6 {
    // Wait for the interval to fire
    interval.tick().await;

    let previous = tokio::time::Instant::now();
    let prev_tokio = tokio::time::Instant::now();
    let current_time: DateTime<Local> = Local::now();
    // Format the date and time as a string
    let current_time_str = current_time.format("%Y-%m-%d %H:%M:%S").to_string();
    // let result = fut.await.unwrap();
    println!(
      "Time {:?} {:?} {:?}, Current Time: {}",
      tokio::time::Instant::now().duration_since(previous),
      // result.duration_since(prev_tokio),
      tokio::time::Instant::now().duration_since(prev_tokio),
      tokio::time::Instant::now(),
      current_time_str,
    );

    // Perform some action
    // println!("Interval tick");
    interval = time::interval(Duration::from_secs(i));
    interval.reset();
  }
}
