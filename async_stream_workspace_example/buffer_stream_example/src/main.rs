use async_io::{block_on, Timer};
use async_stream::stream;
use futures::StreamExt;
use futures_lite::future::yield_now;
use std::time::{Duration, Instant};
fn main() {
  let result = block_on(async {
    stream! {
      loop {
        yield async {
          let start = Instant::now();
          yield_now().await;
          Instant::now().duration_since(start)
        };
      }
    }
    .buffered(5)
    .take(5)
    .then(|d| async move {
      Timer::after(Duration::from_millis(500)).await;
      d
    })
    .collect::<Vec<_>>()
    .await
  });
  dbg!(result);
  // [examples/buffered_stream.rs:26:5] result = [
  //     612.875µs,
  //     501.832917ms,
  //     1.002531209s,
  //     1.503673417s,
  //     2.004864417s,  <---- ???
  // ]
}

// copy from https://gist.github.com/ethe/d12fec2e86efde95a2e1a146940c9c10
// see https://github.com/rust-lang/futures-rs/issues/2387
