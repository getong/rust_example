// copy from [API call tracing in high-loaded asynchronous Rust applications](https://medium.com/@disserman/api-call-tracing-in-high-loaded-asynchronous-rust-applications-bc7b126eb470)
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Request, Response, Server, StatusCode};
use log::{trace, LevelFilter, Log};
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use serde_json::to_value;
use serde_json::Value;
use std::future::Future;
use std::{convert::Infallible, net::SocketAddr};
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio::task::futures::TaskLocalFuture;

tokio::task_local! {
    static TRACE_LOG_TX: Option<mpsc::UnboundedSender<String>>;
}

struct Logger {
  filter: OnceCell<LevelFilter>,
}

impl Log for Logger {
  fn enabled(&self, _metadata: &log::Metadata) -> bool {
    true
  }
  fn log(&self, record: &log::Record) {
    if self.enabled(record.metadata()) {
      if record.level() <= *self.filter.get().unwrap() {
        // write the log record to file or wherever else
      }
      // process call tracing
      let trace_log_tx = TRACE_LOG_TX.try_with(Clone::clone).unwrap_or_default();
      if let Some(tx) = trace_log_tx {
        let _r = tx.send(format!("{}", record.args()));
      }
    }
  }
  fn flush(&self) {}
}

static LOGGER: Logger = Logger {
  filter: OnceCell::new(),
};

#[derive(Serialize)]
struct RpcResponse {
  #[serde(skip_serializing_if = "Option::is_none")]
  result: Option<Value>,
  #[serde(skip_serializing_if = "Option::is_none")]
  error: Option<Value>,
}

async fn method_test(_payload: Value) -> Result<Value, String> {
  Ok(to_value("passed").unwrap())
}

async fn method_ls(payload: Value) -> Result<Value, String> {
  #[derive(Deserialize)]
  #[serde(deny_unknown_fields)]
  struct Params {
    path: String,
  }
  let params = Params::deserialize(payload).map_err(|e| e.to_string())?;
  trace!("listing contents of {}", params.path);
  let result = &Command::new("ls")
    .arg(params.path)
    .output()
    .await
    .map_err(|e| e.to_string())?
    .stdout;
  to_value(std::str::from_utf8(result).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

async fn rpc_call<F, Fut>(f: F, payload: Value) -> RpcResponse
where
  F: Fn(Value) -> Fut,
  Fut: Future<Output = Result<Value, String>>,
{
  match f(payload).await {
    Ok(result) => RpcResponse {
      result: Some(result),
      error: None,
    },
    Err(e) => RpcResponse {
      result: None,
      error: Some(Value::String(e)),
    },
  }
}

fn call_scope<F>(
  trace: bool,
  f: F,
) -> (
  TaskLocalFuture<Option<mpsc::UnboundedSender<String>>, F>,
  Option<mpsc::UnboundedReceiver<String>>,
)
where
  F: Future,
{
  let (tx, rx) = if trace {
    let (tx, rx) = mpsc::unbounded_channel();
    (Some(tx), Some(rx))
  } else {
    (None, None)
  };
  (TRACE_LOG_TX.scope(tx, f), rx)
}

async fn handle(req: Request<Body>) -> Result<Response<Body>, Infallible> {
  let (parts, mut body) = req.into_parts();
  if parts.method == Method::POST {
    if let Some(method) = parts.uri.path().strip_prefix("/api/") {
      let trace = parts
        .headers
        .get("X-Call-Trace")
        .map_or(false, |v| v == "true");
      let payload: Value = serde_json::from_slice(&hyper::body::to_bytes(&mut body).await.unwrap())
        .unwrap_or_default();
      let (response_fut, trace_rx) = call_scope(trace, async move {
        trace!("RPC method: {}", method);
        trace!("RPC payload: {}", payload);
        match method {
          "test" => rpc_call(method_test, payload).await,
          "ls" => rpc_call(method_ls, payload).await,
          _ => RpcResponse {
            result: None,
            error: Some(to_value("invalid method").unwrap()),
          },
        }
      });
      let response = response_fut.await;
      let b = if let Some(mut rx) = trace_rx {
        let mut trace_log = Vec::new();
        while let Some(line) = rx.recv().await {
          trace_log.push(line);
        }
        serde_json::to_string(&vec![
          to_value(response).unwrap(),
          to_value(trace_log).unwrap(),
        ])
        .unwrap()
      } else {
        serde_json::to_string(&response).unwrap()
      };
      return Ok(Response::builder().body(Body::from(b)).unwrap());
    }
  }
  Ok(
    Response::builder()
      .status(StatusCode::BAD_REQUEST)
      .body(Body::empty())
      .unwrap(),
  )
}

#[tokio::main]
async fn main() {
  LOGGER.filter.set(LevelFilter::Info).unwrap();
  log::set_logger(&LOGGER)
    .map(|()| log::set_max_level(LevelFilter::Trace))
    .unwrap();
  let addr = SocketAddr::from(([127, 0, 0, 1], 3999));
  let make_svc = make_service_fn(|_conn| async { Ok::<_, Infallible>(service_fn(handle)) });
  let server = Server::bind(&addr).serve(make_svc);
  server.await.unwrap();
}
