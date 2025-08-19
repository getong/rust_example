use std::{
  future::Future,
  pin::Pin,
  task::{Context, Poll},
  time::{Duration, Instant},
};

struct LoopingFuture {
  counter: u32,
  max_iterations: u32,
  last_yield: Instant,
  yield_interval: Duration,
}

impl LoopingFuture {
  fn new(max_iterations: u32) -> Self {
    Self {
      counter: 0,
      max_iterations,
      last_yield: Instant::now(),
      yield_interval: Duration::from_millis(100),
    }
  }
}

impl Future for LoopingFuture {
  type Output = u32;

  fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    loop {
      if self.counter >= self.max_iterations {
        println!("Future completed with {} iterations", self.counter);
        return Poll::Ready(self.counter);
      }

      if self.last_yield.elapsed() >= self.yield_interval {
        self.counter += 1;
        println!("Iteration {}/{}", self.counter, self.max_iterations);
        self.last_yield = Instant::now();

        cx.waker().wake_by_ref();
        return Poll::Pending;
      }

      self.counter += 1;
      println!("Processing iteration {}", self.counter);

      if self.counter % 5 == 0 {
        cx.waker().wake_by_ref();
        return Poll::Pending;
      }
    }
  }
}

#[tokio::main]
async fn main() {
  println!("Starting custom Future with internal loop");

  let future = LoopingFuture::new(20);
  let result = future.await;

  println!("Future returned: {}", result);

  println!("\nRunning multiple futures concurrently:");
  let future1 = LoopingFuture::new(10);
  let future2 = LoopingFuture::new(15);
  let future3 = LoopingFuture::new(8);

  let (result1, result2, result3) = tokio::join!(future1, future2, future3);

  println!("Results: {}, {}, {}", result1, result2, result3);
}
