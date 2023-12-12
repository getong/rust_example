use tokio::select;
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() {
  let token = CancellationToken::new();
  let child_token = token.child_token();

  let join_handle = tokio::spawn(async move {
    // Wait for either cancellation or a very long time
    select! {
        _ = child_token.cancelled() => {
            // The token was cancelled
            5
        }
        _ = tokio::time::sleep(std::time::Duration::from_secs(9999)) => {
            99
        }
    }
  });

  tokio::spawn(async move {
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    token.cancel();
  });

  assert_eq!(5, join_handle.await.unwrap());
}
