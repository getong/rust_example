use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
  // construct a metrics taskmonitor
  let metrics_monitor = tokio_metrics::TaskMonitor::new();

  // print task metrics every 500ms
  {
    let metrics_monitor = metrics_monitor.clone();
    tokio::spawn(async move {
      for interval in metrics_monitor.intervals() {
        // pretty-print the metric interval
        println!("{:?}", interval);
        // wait 500ms
        tokio::time::sleep(Duration::from_millis(500)).await;
      }
    });
  }

  // instrument some tasks and await them
  // note that the same taskmonitor can be used for multiple tasks
  tokio::join![
    metrics_monitor.instrument(do_work()),
    metrics_monitor.instrument(do_work()),
    metrics_monitor.instrument(do_work())
  ];

  Ok(())
}

async fn do_work() {
  for _ in 0..25 {
    tokio::task::yield_now().await;
    tokio::time::sleep(Duration::from_millis(100)).await;
  }
}

// TaskMetrics { instrumented_count: 3, dropped_count: 0, first_poll_count: 3, total_first_poll_delay: 28.446µs, total_idled_count: 0, total_idle_duration: 0ns, total_scheduled_count: 3, total_scheduled_duration: 56.915µs, total_poll_count: 6, total_poll_duration: 28.884µs, total_fast_poll_count: 6, total_fast_poll_duration: 28.884µs, total_slow_poll_count: 0, total_slow_poll_duration: 0ns, total_short_delay_count: 3, total_long_delay_count: 0, total_short_delay_duration: 56.915µs, total_long_delay_duration: 0ns }
// TaskMetrics { instrumented_count: 0, dropped_count: 0, first_poll_count: 0, total_first_poll_delay: 0ns, total_idled_count: 12, total_idle_duration: 1.217781669s, total_scheduled_count: 24, total_scheduled_duration: 529.71µs, total_poll_count: 24, total_poll_duration: 116.598µs, total_fast_poll_count: 24, total_fast_poll_duration: 116.598µs, total_slow_poll_count: 0, total_slow_poll_duration: 0ns, total_short_delay_count: 22, total_long_delay_count: 2, total_short_delay_duration: 409.374µs, total_long_delay_duration: 120.336µs }
