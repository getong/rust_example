use std::env;

use tracing_subscriber::{EnvFilter, prelude::*};

const DEFAULT_LOG_FILTER: &str = "info";

pub fn init_tracing(tokio_console: bool) {
  if tokio_console {
    let node_name = env::var("LIBP2P_SELF_NAME").unwrap_or_else(|_| "node".to_string());
    let console_bind =
      env::var("TOKIO_CONSOLE_BIND").unwrap_or_else(|_| "127.0.0.1:6669".to_string());
    let fmt_layer = tracing_subscriber::fmt::layer()
      .with_target(true)
      .with_thread_ids(true)
      .with_level(true)
      .with_ansi(false)
      .with_file(true)
      .with_line_number(true)
      .with_filter(env_filter());

    tracing_subscriber::registry()
      .with(
        console_subscriber::ConsoleLayer::builder()
          .with_default_env()
          .spawn(),
      )
      .with(fmt_layer)
      .init();

    println!("{node_name}: tokio-console listening on {console_bind}");
  } else {
    let fmt_layer = tracing_subscriber::fmt::layer()
      .with_target(true)
      .with_thread_ids(true)
      .with_level(true)
      .with_ansi(false)
      .with_file(true)
      .with_line_number(true)
      .with_filter(env_filter());

    tracing_subscriber::registry().with(fmt_layer).init();
  }
}

fn env_filter() -> EnvFilter {
  EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_LOG_FILTER))
}
