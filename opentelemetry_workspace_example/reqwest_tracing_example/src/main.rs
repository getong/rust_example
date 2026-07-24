use std::time::Instant;

use http::Extensions;
use reqwest::{Request, Response};
use reqwest_middleware::{ClientBuilder, Result};
use reqwest_tracing::{
  ReqwestOtelSpanBackend, TracingMiddleware, default_on_request_end, reqwest_otel_span,
};
use tracing::{Level, Span};
use tracing_subscriber::FmtSubscriber;

pub struct TimeTrace;

impl ReqwestOtelSpanBackend for TimeTrace {
  fn on_request_start(req: &Request, extension: &mut Extensions) -> Span {
    extension.insert(Instant::now());
    reqwest_otel_span!(
      name = "example-request",
      req,
      time_elapsed = tracing::field::Empty
    )
  }

  fn on_request_end(span: &Span, outcome: &Result<Response>, extension: &mut Extensions) {
    let time_elapsed = extension.get::<Instant>().unwrap().elapsed().as_millis() as i64;
    default_on_request_end(span, outcome);
    span.record("time_elapsed", &time_elapsed);
  }
}

#[tokio::main]
async fn main() {
  let subscriber = FmtSubscriber::builder()
    .with_max_level(Level::TRACE)
    .finish();

  tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

  run().await;
}

async fn run() {
  let client = ClientBuilder::new(reqwest::Client::new())
    .with(TracingMiddleware::<TimeTrace>::new())
    .build();

  client.get("https://truelayer.com").send().await.unwrap();
}
