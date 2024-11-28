use chrono::Local;
use tokio::{
  runtime::Runtime,
  time::{self, Duration, Instant, MissedTickBehavior},
};

fn now() -> String {
  Local::now().format("%F %T").to_string()
}

fn main() {
  let rt = Runtime::new().unwrap();
  rt.block_on(async {
    println!("before: {}", now());

    let mut intv = time::interval_at(
      Instant::now() + Duration::from_secs(5),
      Duration::from_secs(2),
    );
    intv.set_missed_tick_behavior(MissedTickBehavior::Skip);

    time::sleep(Duration::from_secs(10)).await;

    println!("start: {}", now());
    intv.tick().await;
    println!("tick 1: {}", now());
    intv.tick().await;
    println!("tick 2: {}", now());
    intv.tick().await;
    println!("tick 3: {}", now());
  });
}
