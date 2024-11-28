use std::{env, sync::Arc};

use sentry::{protocol::Event, Level};

const SENTRY_DSN: &str = env!("SENTRY_DSN");

fn before_send(mut event: Event<'static>) -> Option<Event<'static>> {
  if let Some(name) = std::thread::current().name() {
    event.extra.insert("thread.name".to_string(), name.into());
  }

  event.extra.insert("hello".to_string(), "world".into());

  let ex = match event
    .exception
    .values
    .iter()
    .next()
    .and_then(|ex| ex.value.as_ref())
  {
    Some(ex) => ex,
    None => return Some(event),
  };

  // Group events via fingerprint, or ignore

  if ex.starts_with("DBError failed to open the database") {
    event.level = Level::Warning;
  } else if ex.contains("SqliteFailure") {
    event.level = Level::Warning;
  } else if ex.starts_with("DBError the database version")
    || ex.contains("kind: AddrInUse")
    || ex.contains("kind: AddrNotAvailable")
    || ex.contains("IO error: No space left")
  {
    // ignore
    return None;
  }

  Some(event)
}

fn main() {
  let _sentry = sentry::init((
    SENTRY_DSN,
    sentry::ClientOptions {
      before_send: Some(Arc::new(Box::new(before_send))),
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
