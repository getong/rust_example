use std::time::Duration;

use tokio::time::interval;

#[tokio::main]
async fn main() {
  let mut tick = interval(Duration::from_millis(500));
  let mut longer_tick = interval(Duration::from_millis(1000));

  loop {
    tokio::select! {
        // _ = signal::ctrl_c() => {
        //     println!("received interrupt signal");
        //     break;
        // },
        _ = longer_tick.tick() => {
            println!("longer tick");
        },
        _ = tick.tick() => {
            println!("sleeping");
            sleep().await;
        },
    }
  }
}

async fn sleep() {
  let mut idx = 0;
  loop {
    let time = idx.min(5);
    println!("Sleeping for {} s", time);
    tokio::time::sleep(Duration::from_secs(time)).await;
    idx += 1;
  }
}
