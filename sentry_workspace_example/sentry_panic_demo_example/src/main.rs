const SENTRY_DSN: &str = "your-sentry-dsn";

fn main() {
  let _sentry = sentry::init((
    SENTRY_DSN,
    sentry::ClientOptions {
      release: sentry::release_name!(),
      debug: true,
      ..Default::default()
    },
  ));
  sentry::configure_scope(|scope| {
    scope.set_fingerprint(Some(["a-message"].as_ref()));
    scope.set_tag("foo", "bar");
  });

  sentry::capture_message("This is recorded as a warning now", sentry::Level::Warning);
}
