use std::path::PathBuf;

use anyhow::{Error, Result};
use futures_util::TryFutureExt;
use http_body_util::{BodyExt, Full};
use hyper::{
  Method, Request, Response,
  body::{Bytes, Incoming},
  header::CONTENT_LENGTH,
  http::uri::Authority,
  service::service_fn,
};
use hyper_util::{
  client::legacy::{Client, connect::HttpConnector},
  rt::{TokioExecutor, TokioIo},
  server::conn::auto,
};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use tokio::net::TcpListener;

const TAURI_OPTIONS: &str = "tauri:options";

const HELP: &str = "\
USAGE: tauri-driver [FLAGS] [OPTIONS]

FLAGS:
  -h, --help              Prints help information

OPTIONS:
  --port NUMBER           Sets the tauri-driver intermediary port
  --native-port NUMBER    Sets the port of the underlying WebDriver
  --native-host HOST      Sets the host of the underlying WebDriver (Linux only)
  --native-driver PATH    Sets the path to the native WebDriver binary
";

#[derive(Debug, Clone)]
pub struct Args {
  pub port: u16,
  pub native_port: u16,
  pub native_host: String,
  pub native_driver: Option<PathBuf>,
}

impl From<pico_args::Arguments> for Args {
  fn from(mut args: pico_args::Arguments) -> Self {
    // if the user wanted help, we don't care about parsing the rest of the args
    if args.contains(["-h", "--help"]) {
      println!("{}", HELP);
      std::process::exit(0);
    }

    let native_driver = match args.opt_value_from_str("--native-driver") {
      Ok(native_driver) => native_driver,
      Err(e) => {
        eprintln!("Error while parsing option --native-driver: {}", e);
        std::process::exit(1);
      }
    };

    let parsed = Args {
      port: args.value_from_str("--port").unwrap_or(4444),
      native_port: args.value_from_str("--native-port").unwrap_or(4445),
      native_host: args
        .value_from_str("--native-host")
        .unwrap_or(String::from("127.0.0.1")),
      native_driver,
    };

    // be strict about accepting args, error for anything extraneous
    let rest = args.finish();
    if !rest.is_empty() {
      eprintln!("Error: unused arguments left: {:?}", rest);
      eprintln!("{}", HELP);
      std::process::exit(1);
    }

    parsed
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TauriOptions {
  application: PathBuf,
  #[serde(default)]
  args: Vec<String>,
}

impl TauriOptions {
  fn into_native_object(self) -> Map<String, Value> {
    let mut map = Map::new();
    map.insert(
      "webkitgtk:browserOptions".into(),
      json!({"binary": self.application, "args": self.args}),
    );
    map
  }
}

async fn handle(
  client: Client<HttpConnector, Full<Bytes>>,
  req: Request<Incoming>,
  args: Args,
) -> Result<Response<Incoming>, Error> {
  // manipulate a new session to convert options to the native driver format
  let new_req: Request<Full<Bytes>> =
    if let (&Method::POST, "/session") = (req.method(), req.uri().path()) {
      let (mut parts, body) = req.into_parts();

      // get the body from the future stream and parse it as json
      let body = body.collect().await?.to_bytes().to_vec();
      let json: Value = serde_json::from_slice(&body)?;

      // manipulate the json to convert from tauri option to native driver options
      let json = map_capabilities(json);

      // serialize json and update the content-length header to be accurate
      let bytes = serde_json::to_vec(&json)?;
      parts.headers.insert(CONTENT_LENGTH, bytes.len().into());

      Request::from_parts(parts, Full::new(bytes.into()))
    } else {
      let (parts, body) = req.into_parts();

      let body = body.collect().await?.to_bytes().to_vec();

      Request::from_parts(parts, Full::new(body.into()))
    };

  client
    .request(forward_to_native_driver(new_req, args)?)
    .err_into()
    .await
}

/// Transform the request to a request for the native webdriver server.
fn forward_to_native_driver(
  mut req: Request<Full<Bytes>>,
  args: Args,
) -> Result<Request<Full<Bytes>>, Error> {
  let host: Authority = {
    let headers = req.headers_mut();
    headers.remove("host").expect("hyper request has host")
  }
  .to_str()?
  .parse()?;

  let path = req
    .uri()
    .path_and_query()
    .expect("hyper request has uri")
    .clone();

  let uri = format!(
    "http://{}:{}{}",
    host.host(),
    args.native_port,
    path.as_str()
  );

  let (mut parts, body) = req.into_parts();
  parts.uri = uri.parse()?;
  Ok(Request::from_parts(parts, body))
}

/// only happy path for now, no errors
fn map_capabilities(mut json: Value) -> Value {
  let mut native = None;
  if let Some(capabilities) = json.get_mut("capabilities") {
    if let Some(always_match) = capabilities.get_mut("alwaysMatch") {
      if let Some(always_match) = always_match.as_object_mut() {
        if let Some(tauri_options) = always_match.remove(TAURI_OPTIONS) {
          if let Ok(options) = serde_json::from_value::<TauriOptions>(tauri_options) {
            native = Some(options.into_native_object());
          }
        }

        if let Some(native) = native.clone() {
          always_match.extend(native);
        }
      }
    }
  }

  if let Some(native) = native {
    if let Some(desired) = json.get_mut("desiredCapabilities") {
      if let Some(desired) = desired.as_object_mut() {
        desired.remove(TAURI_OPTIONS);
        desired.extend(native);
      }
    }
  }

  json
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let args: Args = pico_args::Arguments::from_env().into();
  let address = std::net::SocketAddr::from(([127, 0, 0, 1], args.port));

  // the client we use to proxy requests to the native webdriver
  let client = Client::builder(TokioExecutor::new())
    .http1_preserve_header_case(true)
    .http1_title_case_headers(true)
    .retry_canceled_requests(false)
    .build_http();

  // set up a http1 server that uses the service we just created
  let srv = async move {
    if let Ok(listener) = TcpListener::bind(address).await {
      loop {
        let client = client.clone();
        let args = args.clone();
        if let Ok((stream, _)) = listener.accept().await {
          let io = TokioIo::new(stream);

          tokio::task::spawn(async move {
            if let Err(err) = auto::Builder::new(TokioExecutor::new())
              .http1()
              .title_case_headers(true)
              .preserve_header_case(true)
              .serve_connection(
                io,
                service_fn(|request| handle(client.clone(), request, args.clone())),
              )
              .await
            {
              println!("Error serving connection: {:?}", err);
            }
          });
        } else {
          println!("accept new stream fail, ignore here");
        }
      }
    } else {
      println!("can not listen to address: {:?}", address);
    }
  };

  srv.await;

  Ok(())
}
