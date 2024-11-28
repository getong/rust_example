use std::{future::Future, pin::Pin};

async fn async_square(num: i32) -> i32 {
  tokio::time::sleep(std::time::Duration::from_secs(1)).await;
  num * num
}

fn apply_async_function<F, Fut>(f: F, value: i32) -> Pin<Box<dyn Future<Output = i32>>>
where
  F: Fn(i32) -> Fut,
  Fut: Future<Output = i32> + 'static,
{
  Box::pin(f(value))
}

#[tokio::main]
async fn main() {
  let result = apply_async_function(async_square, 5).await;
  println!("The square of 5 is {}", result);
}
