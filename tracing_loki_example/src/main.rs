use std::{process, time::Duration};

use tracing::{debug, error, info, instrument, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use url::Url;

#[tokio::main]
async fn main() -> Result<(), tracing_loki::Error> {
  let (layer, task) = tracing_loki::builder()
    .label("host", "mine")?
    .label("service", "demo_app")?
    .extra_field("pid", format!("{}", process::id()))?
    .build_url(Url::parse("http://127.0.0.1:3100").unwrap())?;

  // We need to register our layer with `tracing`.
  tracing_subscriber::registry()
    .with(layer)
    // Also log to stdout for visibility without ANSI colors
    // Enable file name, line number, and target information
    .with(
      tracing_subscriber::fmt::Layer::new()
        .with_ansi(false)
        .with_file(true)
        .with_line_number(true)
        .with_target(true)
        .with_thread_ids(true)
        .with_thread_names(true),
    )
    .init();

  // The background task needs to be spawned so the logs actually get
  // delivered.
  tokio::spawn(task);

  info!(
    task = "tracing_setup",
    result = "success",
    "tracing successfully set up",
  );

  // Demonstrate various async logging capabilities
  info!("Starting async demo application");

  // Call different async functions to show logging
  process_user_request("user_123", 42).await;

  // Run multiple tasks concurrently
  run_concurrent_tasks().await;

  // Simulate error handling
  handle_errors().await;

  // Process data with different log levels
  process_data_pipeline().await;

  info!("Application shutting down gracefully");

  // Give some time for logs to be sent
  tokio::time::sleep(Duration::from_secs(2)).await;

  Ok(())
}

#[instrument(skip(user_id, count))]
async fn process_user_request(user_id: &str, count: u32) {
  info!(
    user_id = user_id,
    request_count = count,
    "Processing user request"
  );

  tokio::time::sleep(Duration::from_millis(100)).await;

  debug!(user_id = user_id, "Fetching user profile");
  tokio::time::sleep(Duration::from_millis(50)).await;

  info!(
    user_id = user_id,
    items_processed = count,
    "User request completed successfully"
  );
}

#[instrument]
async fn run_concurrent_tasks() {
  info!("Starting concurrent task execution");

  let handles: Vec<_> = (0 .. 5)
    .map(|i| {
      tokio::spawn(async move {
        let task_id = i;
        info!(task_id, "Async task started");

        tokio::time::sleep(Duration::from_millis(50 * i)).await;

        debug!(task_id, delay_ms = 50 * i, "Task completed work");

        task_id * 2
      })
    })
    .collect();

  for (idx, handle) in handles.into_iter().enumerate() {
    match handle.await {
      Ok(result) => info!(task_idx = idx, result, "Task finished successfully"),
      Err(e) => error!(task_idx = idx, error = %e, "Task failed"),
    }
  }

  info!("All concurrent tasks completed");
}

#[instrument]
async fn handle_errors() {
  info!("Demonstrating error logging");

  // Simulate some warnings
  for i in 0 .. 3 {
    warn!(
      iteration = i,
      threshold = 100,
      "Resource usage approaching limit"
    );
    tokio::time::sleep(Duration::from_millis(30)).await;
  }

  // Simulate an error scenario
  match risky_operation().await {
    Ok(value) => info!(result = value, "Risky operation succeeded"),
    Err(e) => error!(error = %e, "Risky operation failed"),
  }
}

#[instrument]
async fn risky_operation() -> Result<i32, &'static str> {
  debug!("Attempting risky operation");
  tokio::time::sleep(Duration::from_millis(100)).await;

  // Simulate success this time
  Ok(42)
}

#[instrument]
async fn process_data_pipeline() {
  info!("Starting data pipeline");

  let stages = vec!["ingestion", "validation", "transformation", "storage"];

  for (idx, stage) in stages.iter().enumerate() {
    info!(
      stage = stage,
      stage_number = idx + 1,
      total_stages = stages.len(),
      "Processing pipeline stage"
    );

    tokio::time::sleep(Duration::from_millis(80)).await;

    let records_processed = (idx + 1) * 100;
    debug!(
      stage = stage,
      records = records_processed,
      "Stage completed"
    );
  }

  info!(
    total_records = 1000,
    duration_ms = 320,
    "Data pipeline completed successfully"
  );
}
