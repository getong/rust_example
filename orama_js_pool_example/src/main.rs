use std::time::Duration;

use orama_js_pool::{ExecOption, JSPoolExecutor, JSRunnerError};

static CODE_ASYNC_SUM: &str = r#"
async function async_sum(a, b) {
    await new Promise(resolve => setTimeout(resolve, 100));
    return a + b
}
export default { async_sum };
"#;

#[tokio::main]
async fn main() -> Result<(), JSRunnerError> {
  // Create a pool with 10 JS engines, running the code above
  let pool = JSPoolExecutor::<Vec<u8>, u8>::new(
    CODE_ASYNC_SUM.to_string(),
    10,                         // number of engines
    None,                       // no http domain restriction on startup
    Duration::from_millis(200), // startup timeout
    true,                       // is_async
    "async_sum".to_string(),    // function name to call
  )
  .await?;

  let params = vec![1, 2];
  let result = pool
    .exec(
      params, // input parameter
      None,   // no stdout stream (set to Some(...) to capture stdout/stderr)
      ExecOption {
        timeout: Duration::from_millis(200), // timeout
        allowed_hosts: None,
      },
    )
    .await?;

  println!("async_sum(1, 2) == {}", result);
  assert_eq!(result, 3);
  Ok(())
}
