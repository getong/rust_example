use rquickjs::{async_with, AsyncContext, AsyncRuntime};
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main]
async fn main() {
  // Initialize AsyncRuntime and AsyncContext
  let rt = AsyncRuntime::new().unwrap();
  let ctx = AsyncContext::full(&rt).await.unwrap();

  // Call the async_with! macro to execute the asynchronous block
  async_with!(&ctx => |ctx| {
    // Enter a loop to evaluate JavaScript code repeatedly
    loop {
      let code_str = "1 + 1";
      let res: i32 = ctx.eval(code_str).unwrap();
      println!("res is {}", res);
      assert_eq!(res, 2i32);

      // Sleep for a duration before the next iteration
      sleep(Duration::from_secs(1)).await;
    }
  })
  .await;
}
