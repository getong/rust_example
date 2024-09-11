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

  let handle = std::thread::spawn(|| {
    sentry::start_session();
    std::thread::sleep(std::time::Duration::from_secs(3));
    panic!("oh no!");
  });

  sentry::start_session();

  sentry::capture_message(
    "anything with a level >= Error will increase the error count",
    sentry::Level::Error,
  );

  let err = "NaN".parse::<usize>().unwrap_err();
  sentry::capture_error(&err);

  std::thread::sleep(std::time::Duration::from_secs(2));

  sentry::end_session();

  if let Err(e) = handle.join() {
    eprintln!("Thread panicked: {:?}", e);
  }
}
