use std::time::Duration;

use tokio::time::sleep;
use tokio_tasks_shutdown::*;

#[tokio::main]
async fn main() {
  // By default this will catch Ctrl+C.
  // You may have your tasks return your own error type.
  let tasks: TasksMainHandle<anyhow::Error> = TasksBuilder::default()
    .timeouts(
      Some(Duration::from_secs(2)),
      Some(Duration::from_millis(500)),
    )
    .build();

  // Spawn tasks
  tasks
    .spawn("gracefully_shutting_down_task", |tasks_handle| async move {
      loop {
        match tasks_handle
          .on_shutdown_or({
            // Simulating another future running concurrently,
            // e.g. listening on a channel...
            sleep(Duration::from_millis(100))
          })
          .await
        {
          ShouldShutdownOr::ShouldShutdown => {
            // We have been kindly asked to shutdown, let's exit
            break;
          }
          ShouldShutdownOr::ShouldNotShutdown(_res) => {
            // Got result of channel listening
          }
        }
      }
      Ok(())
      // Note that if a task were to error, graceful shutdown would be initiated.
      // This behavior can be disabled.
    })
    .unwrap();
  // Note that calls can be chained since `spawn` returns `&TasksHandle`

  // Let's simulate a Ctrl+C after some time
  let tasks_handle: TasksHandle<_> = tasks.handle();
  tokio::task::spawn(async move {
    sleep(Duration::from_millis(150)).await;
    tasks_handle.start_shutdown();
  });

  // Let's make sure there were no errors
  tasks.join_all().await.unwrap();

  let test_duration = Duration::from_millis(146);
  // Make sure we have shut down when expected
  assert!(test_duration > Duration::from_millis(145) && test_duration < Duration::from_millis(155));
}
