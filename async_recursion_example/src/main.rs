use async_recursion::async_recursion;
#[async_recursion]
async fn async_fibo(n: u64) -> u64 {
  match n {
    0 => 0,
    1 => 1,
    _ => async_fibo(n - 1).await + async_fibo(n - 2).await,
  }
}

#[tokio::main]
async fn main() {
  // println!("Hello, world!");
  println!("async_fibo(6): {}", async_fibo(6).await);
}
