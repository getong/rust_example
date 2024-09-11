use std::env;

const SENTRY_DSN: &str = env!("SENTRY_DSN");

fn main() {
  let _sentry = sentry::init((
    SENTRY_DSN,
    sentry::ClientOptions {
      release: sentry::release_name!(),
      debug: true,
      auto_session_tracking: true,
      attach_stacktrace: true,
      ..Default::default()
    },
  ));

  // Send a message to Sentry
  // sentry::capture_message("This is a test message before crashing", sentry::Level::Info);

  // Simulate a crash using unwrap()
  let maybe_number: Result<i32, &str> = Err("This will crash");
  let _number = maybe_number.unwrap(); // This will panic

  // This line won't be reached due to the panic above
  // sentry::capture_message("This will not be reached", sentry::Level::Info);

  // Sentry will automatically flush pending events when the process exits
}
