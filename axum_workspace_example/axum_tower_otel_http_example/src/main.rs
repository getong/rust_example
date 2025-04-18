use std::{
  future::{Future, IntoFuture},
  pin::Pin,
  task::{Context, Poll, ready},
  time::Duration,
};

use axum::{Router, http::Request, routing::get};
use http_body_util::{BodyExt, Empty};
use opentelemetry::trace::TracerProvider;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use pin_project::pin_project;
use tower::{Service, ServiceBuilder, ServiceExt};
use tower_otel::{metrics, trace};
use tracing::Level;
use tracing_subscriber::{
  Layer, filter::LevelFilter, fmt::format::FmtSpan, layer::SubscriberExt, util::SubscriberInitExt,
};

#[tokio::main]
async fn main() {
  opentelemetry::global::set_text_map_propagator(TraceContextPropagator::new());

  const PKG_NAME: &str = env!("CARGO_PKG_NAME");
  const PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

  let resource = opentelemetry_sdk::Resource::builder()
    .with_service_name(PKG_NAME)
    .with_attribute(opentelemetry::KeyValue::new("service.version", PKG_VERSION))
    .build();

  let span_exporter = opentelemetry_otlp::SpanExporter::builder()
    .with_tonic()
    .build()
    .unwrap();

  let tracer_provider = opentelemetry_sdk::trace::SdkTracerProvider::builder()
    .with_sampler(opentelemetry_sdk::trace::Sampler::AlwaysOn)
    .with_resource(resource.clone())
    .with_batch_exporter(span_exporter)
    .build();

  let telemetry = tracing_opentelemetry::layer()
    .with_tracer(tracer_provider.tracer("default_tracer"))
    .with_tracked_inactivity(true)
    .with_filter(LevelFilter::TRACE);

  let fmt = tracing_subscriber::fmt::layer()
    .with_level(true)
    .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE);

  tracing_subscriber::registry()
    .with(LevelFilter::from_level(Level::TRACE))
    .with(telemetry)
    .with(fmt)
    .init();

  let metrics_exporter = opentelemetry_otlp::MetricExporter::builder()
    .with_tonic()
    .build()
    .unwrap();

  let metrics_readers = opentelemetry_sdk::metrics::PeriodicReader::builder(metrics_exporter)
    .with_interval(Duration::from_secs(2))
    .build();

  let meter_provider = opentelemetry_sdk::metrics::SdkMeterProvider::builder()
    .with_reader(metrics_readers)
    .with_resource(resource)
    .build();

  opentelemetry::global::set_meter_provider(meter_provider.clone());
  let meter = opentelemetry::global::meter(PKG_NAME);

  let app = Router::new()
    .route("/", get(|| async { "Hello, World!" }))
    .layer(trace::HttpLayer::server(Level::DEBUG))
    .layer(metrics::HttpLayer::server(&meter));
  let listener = tokio::net::TcpListener::bind("[::1]:3000").await.unwrap();
  let server = axum::serve(listener, app).into_future();
  tokio::spawn(server);

  let tcp = tokio::net::TcpStream::connect("[::1]:3000").await.unwrap();
  let io = TokioIo(tcp);
  let (request_sender, connection) = hyper::client::conn::http1::handshake(io).await.unwrap();
  tokio::spawn(connection);
  let mut client = ServiceBuilder::new()
    .layer(trace::HttpLayer::client(Level::DEBUG))
    .layer(metrics::HttpLayer::client(&meter))
    .service(Client(request_sender));

  let req = Request::get("http://[::1]:3000")
    .body(Empty::<&[u8]>::new())
    .unwrap();
  let res = client.ready().await.unwrap().call(req).await.unwrap();
  let body = res.collect().await.unwrap().to_bytes();
  let body = std::str::from_utf8(&body).unwrap();
  tracing::info!("received '{}'", body);

  meter_provider.shutdown().unwrap();
  tracer_provider.shutdown().unwrap();
}

struct Client<B>(hyper::client::conn::http1::SendRequest<B>);

impl<B> tower::Service<hyper::Request<B>> for Client<B>
where
  B: hyper::body::Body + 'static,
{
  type Response = hyper::Response<hyper::body::Incoming>;
  type Error = hyper::Error;
  type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

  fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
    self.0.poll_ready(cx)
  }

  fn call(&mut self, req: hyper::Request<B>) -> Self::Future {
    Box::pin(self.0.send_request(req))
  }
}

#[pin_project]
struct TokioIo<T>(#[pin] T);

impl<T> hyper::rt::Read for TokioIo<T>
where
  T: tokio::io::AsyncRead,
{
  fn poll_read(
    self: Pin<&mut Self>,
    cx: &mut Context<'_>,
    mut buf: hyper::rt::ReadBufCursor<'_>,
  ) -> Poll<Result<(), std::io::Error>> {
    let n = {
      let this = self.project();
      let mut buf = tokio::io::ReadBuf::uninit(unsafe { buf.as_mut() });
      ready!(this.0.poll_read(cx, &mut buf))?;
      buf.filled().len()
    };
    unsafe { buf.advance(n) };
    Poll::Ready(Ok(()))
  }
}

impl<T> hyper::rt::Write for TokioIo<T>
where
  T: tokio::io::AsyncWrite,
{
  fn poll_write(
    self: Pin<&mut Self>,
    cx: &mut Context<'_>,
    buf: &[u8],
  ) -> Poll<Result<usize, std::io::Error>> {
    let this = self.project();
    this.0.poll_write(cx, buf)
  }

  fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
    let this = self.project();
    this.0.poll_flush(cx)
  }

  fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
    let this = self.project();
    this.0.poll_shutdown(cx)
  }

  fn is_write_vectored(&self) -> bool {
    self.0.is_write_vectored()
  }

  fn poll_write_vectored(
    self: Pin<&mut Self>,
    cx: &mut Context<'_>,
    bufs: &[std::io::IoSlice<'_>],
  ) -> Poll<Result<usize, std::io::Error>> {
    let this = self.project();
    this.0.poll_write_vectored(cx, bufs)
  }
}
