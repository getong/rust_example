const SENTRY_DSN: &str = "your-sentry-dsn";

fn main() {
  let _guard = sentry::init((
    SENTRY_DSN,
    sentry::ClientOptions {
      release: sentry::release_name!(),
      ..Default::default()
    },
  ));

  println!("Hello, world!");
  // Sentry will capture this
  panic!("Everything is on fire!");
}
