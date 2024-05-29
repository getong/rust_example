use async_recursion::async_recursion;

#[async_recursion]
async fn fib(n : u32) -> u32 {
  match n {
    0 | 1 => 1,
    _ => fib(n-1).await + fib(n-2).await
  }
}

#[tokio::main]
async fn main() {
  let sum = fib(10).await;
  println!("Sum: {}", sum);
}
