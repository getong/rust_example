use std::time::Duration;
use tokio_graceful::Shutdown;

#[tokio::main]
async fn main() {
  // most users can just use `Shutdown::default()` to initiate
  // shutdown upon either Sigterm or CTRL+C (Sigkill).
  let signal = tokio::time::sleep(std::time::Duration::from_millis(100));
  let shutdown = Shutdown::new(signal);

  // you can use shutdown to spawn tasks that will
  // include a guard to prevent the shutdown from completing
  // aslong as these tasks are open
  shutdown.spawn_task(async {
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
  });
  // or spawn a function such that you have access to the guard coupled to the task
  shutdown.spawn_task_fn(|guard| async move {
    // let guard2 = guard.clone();
    guard.cancelled().await;
  });

  // this guard isn't dropped, but as it's a weak guard
  // it has no impact on the ref count of the common tokens.
  let guard_weak = shutdown.guard_weak();

  // this guard needs to be dropped as otherwise the shutdown is prevented;
  let guard = shutdown.guard();
  drop(guard);

  // guards can be downgraded to weak guards, to not have it be counted any longer in the ref count
  let weak_guard_2 = shutdown.guard().downgrade();

  // guards (weak or not) are cancel safe
  tokio::select! {
      _ = tokio::time::sleep(std::time::Duration::from_millis(10)) => {},
      _ = weak_guard_2.into_cancelled() => {},
  }

  // you can also wait to shut down without any timeout limit
  // `shutdown.shutdown().await;`
  shutdown
    .shutdown_with_limit(Duration::from_secs(60))
    .await
    .unwrap();

  // once a shutdown is triggered the ::cancelled() fn will immediately return true,
  // forever, not just once. Even after shutdown process is completely finished.
  guard_weak.cancelled().await;

  // weak guards can be upgraded to regular guards to take into account for ref count
  let guard = guard_weak.upgrade();
  // even this one however will know it was cancelled
  guard.cancelled().await;
}
