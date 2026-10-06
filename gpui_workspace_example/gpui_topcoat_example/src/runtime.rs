//! Bridge Tokio I/O tasks back to GPUI without blocking its main thread.
use std::{future::Future, sync::OnceLock};

use tokio::{runtime::Runtime, task::JoinHandle};

fn runtime() -> &'static Runtime {
  static RUNTIME: OnceLock<Runtime> = OnceLock::new();
  RUNTIME.get_or_init(|| {
    tokio::runtime::Builder::new_multi_thread()
      .thread_name("topcoat-async")
      .enable_all()
      .build()
      .expect("Failed to create Tokio runtime")
  })
}

/// Initialize the process-wide runtime before opening desktop windows.
/// Lazy initialization also supports GPUI tests that do not call main.
pub fn init() {
  let _ = runtime();
}

struct AbortOnDrop<T>(JoinHandle<T>);

impl<T> Drop for AbortOnDrop<T> {
  fn drop(&mut self) {
    self.0.abort();
  }
}

/// Await this future in cx.spawn and update the entity there.
/// Dropping the future cancels the Tokio task, including when never polled.
/// Cancellation cannot undo a mutation already committed by the server.
pub fn spawn<T: Send + 'static>(
  work: impl Future<Output = Result<T, String>> + Send + 'static,
) -> impl Future<Output = Result<T, String>> {
  let mut task = AbortOnDrop(runtime().spawn(work));
  async move {
    (&mut task.0).await.map_err(|error| {
      format!("Background request failed: {error}. Refresh before retrying a change.")
    })?
  }
}

#[cfg(test)]
mod tests {
  use std::time::Duration;

  use super::*;

  #[tokio::test]
  async fn returns_results_and_reports_panics() {
    assert_eq!(spawn(async { Ok(42) }).await.unwrap(), 42);
    let error = spawn::<()>(async { panic!("test task panic") })
      .await
      .unwrap_err();
    assert!(error.contains("Background request failed"));
  }

  #[tokio::test]
  async fn dropping_unpolled_bridge_cancels_running_work() {
    let (started, ready) = tokio::sync::oneshot::channel();
    let (finished, cancelled) = tokio::sync::oneshot::channel::<()>();
    let task = spawn(async move {
      let _finished = finished;
      started.send(()).unwrap();
      std::future::pending::<()>().await;
      Ok(())
    });
    tokio::time::timeout(Duration::from_secs(5), ready)
      .await
      .unwrap()
      .unwrap();
    drop(task);
    assert!(
      tokio::time::timeout(Duration::from_secs(5), cancelled)
        .await
        .unwrap()
        .is_err()
    );
  }
}
