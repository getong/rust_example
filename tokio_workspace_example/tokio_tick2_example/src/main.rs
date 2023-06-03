use futures::{stream, StreamExt}; // 0.3.13
use std::time::{Duration, Instant};
use tokio::time; // 1.3.0

#[tokio::main]
async fn main() {
    let interval = time::interval(Duration::from_millis(10));

    let forever = stream::unfold(interval, |mut interval| async {
        interval.tick().await;
        do_something().await;
        Some(((), interval))
    });

    let _now = Instant::now();
    forever.for_each(|_| async {}).await;
}

async fn do_something() {
    eprintln!("do_something");
}
