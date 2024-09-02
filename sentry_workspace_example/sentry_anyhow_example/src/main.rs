const SENTRY_DSN: &str = "your-sentry-dsn";

fn execute() -> anyhow::Result<usize> {
  let parsed = "NaN".parse()?;
  Ok(parsed)
}

fn main() {
  let _sentry = sentry::init((
    SENTRY_DSN,
    sentry::ClientOptions {
      release: sentry::release_name!(),
      debug: true,
      ..Default::default()
    },
  ));

  if let Err(err) = execute() {
    println!("error: {err}");
    sentry_anyhow::capture_anyhow(&err);
  }
}
