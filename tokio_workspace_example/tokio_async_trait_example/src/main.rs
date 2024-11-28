// copy from https://blog.rust-lang.org/inside-rust/2022/11/17/async-fn-in-trait-nightly.html

// #![allow(incomplete_features)]
// #![feature(async_fn_in_trait)]

use std::{fmt::Display, future};

trait AsyncIterator {
  type Item;
  async fn next(&mut self) -> Option<Self::Item>;
}

struct Countdown(usize);
impl AsyncIterator for Countdown {
  type Item = usize;
  async fn next(&mut self) -> Option<usize> {
    let val = self.0;
    self.0 = future::ready(val.checked_sub(1)).await?;
    Some(val)
  }
}

async fn print_all<I: AsyncIterator>(mut count: I)
where
  I::Item: Display,
{
  while let Some(x) = count.next().await {
    println!("{x}");
  }
}

async fn do_something() {
  let iter = Countdown(10);
  tokio::spawn(print_all(iter)).await.unwrap();
}

#[tokio::main]
async fn main() {
  do_something().await;
}
