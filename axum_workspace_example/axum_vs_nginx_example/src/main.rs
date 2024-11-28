use std::{collections::HashMap, convert::Infallible, fs, net::SocketAddr, path::PathBuf};

use anyhow::{Context, Result};
use axum::{
  body::Body,
  extract::{Host, State},
  handler::{Handler, HandlerWithoutStateExt},
  http::{Request, Response, StatusCode, Uri},
  response::{IntoResponse, Redirect},
  Router,
};
use hyper_util::{client::legacy::connect::HttpConnector, rt::TokioExecutor};
use log::debug;
use rustls_acme::{caches::DirCache, AcmeConfig};
use tokio_stream::StreamExt;
use tower::{util::BoxCloneService, ServiceExt};
use tower_http::{services::ServeDir, validate_request::ValidateRequestHeaderLayer};
type Client = hyper_util::client::legacy::Client<HttpConnector, Body>;

#[tokio::main]
async fn main() -> Result<()> {
  let site1_svc = Router::new().nest_service("/", ServeDir::new("/tmp/serve_dir"));
  let site2_svc = Router::new().nest_service("/", ServeDir::new("/tmp/serve_dir2"));

  let debug_mode = !std::env::args().any(|x| x == "--production");
  let (name_site1, name_site2, external_app) = if debug_mode {
    (
      "site1.localhost:3000".to_string(),
      "site2.localhost:3000".to_string(),
      "ext.localhost:3000".to_string(),
    )
  } else {
    (
      "site1.my.domain".to_string(),
      "site2.my.domain".to_string(),
      "app.my.domain".to_string(),
    )
  };

  let client: Client = hyper_util::client::legacy::Client::<(), ()>::builder(TokioExecutor::new())
    .build(HttpConnector::new());
  let rev_proxy_svc = Router::new()
    .nest_service(
      "/",
      (|state, req| reverse_proxy_http_handler(3001, state, req)).with_state(client),
    )
    .layer(ValidateRequestHeaderLayer::basic("user", "super safe pw"));

  let hostname_router = mk_hostname_router(
    [
      (name_site1, site1_svc),
      (name_site2, site2_svc),
      (external_app, rev_proxy_svc),
    ]
    .into(),
  );

  let app = Router::new().nest_service("/", hostname_router);

  if debug_mode {
    server_locally(app, 3000).await.context("Serving locally")?;
  } else {
    serve_with_tls(
      app,
      ["site1.my.domain", "site2.my.domain", "ext.my.domain"],
      "your.mail@something.org",
      "/path/to/cache",
    )
    .await
    .context("Serving with TLS")?;
  }

  Ok(())
}

async fn reverse_proxy_http_handler(
  port: u16,
  State(client): State<Client>,
  mut req: Request<Body>,
) -> Result<Response<Body>, StatusCode> {
  // extract the query
  let path = req.uri().path();
  let path_query = req
    .uri()
    .path_and_query()
    .map(|v| v.as_str())
    .unwrap_or(path);

  // construct the new uri query from the port and the query
  let uri = format!("http://127.0.0.1:{port}{path_query}");

  // inject the new uri into the request
  *req.uri_mut() = Uri::try_from(uri).unwrap();

  // hand off the request
  Ok(
    client
      .request(req)
      .await
      .map_err(|_| StatusCode::BAD_REQUEST)?
      .into_response(),
  )
}

pub async fn serve_with_tls(
  app: Router,
  domains: impl IntoIterator<Item = impl AsRef<str>>,
  email_for_lets_encryp: &str,
  cert_cache_dir: impl Into<PathBuf>,
) -> Result<()> {
  // create a cache for the certificates
  let ccache: PathBuf = cert_cache_dir.into();
  if !ccache.exists() {
    fs::create_dir_all(&ccache).context("Creating Cache Dir")?;
  }

  // pass the configuration to AcmeConfig
  let mut state = AcmeConfig::new(domains)
    .contact([format!("mailto:{email_for_lets_encryp}")])
    .cache(DirCache::new(ccache))
    .directory_lets_encrypt(true)
    .state();

  // set everything up as required
  let acceptor = state.axum_acceptor(state.default_rustls_config());

  tokio::spawn(async move {
    loop {
      match state.next().await.unwrap() {
        Ok(ok) => log::info!("event: {ok:?}"),
        Err(err) => log::error!("error: {err}"),
      }
    }
  });

  let addr = SocketAddr::from(([0, 0, 0, 0], 443));
  let tls_server = axum_server::bind(addr)
    .acceptor(acceptor)
    .serve(app.into_make_service());
  let redirect_server = mk_redirect_server();
  Ok(tokio::try_join!(tls_server, redirect_server).map(|_| ())?)
}

async fn mk_redirect_server() -> std::io::Result<()> {
  fn make_https(host: String, uri: Uri) -> Result<Uri, Box<dyn std::error::Error>> {
    debug!("incoming request to {host}{uri}");
    let mut parts = uri.into_parts();
    debug!("request parts: {parts:?}");
    parts.scheme = Some(axum::http::uri::Scheme::HTTPS);

    if parts.path_and_query.is_none() {
      parts.path_and_query = Some("/".parse().unwrap());
    }

    parts.authority = Some(host.parse()?);
    let new_uri = Uri::from_parts(parts)?;
    debug!("redirected to {new_uri}");
    Ok(new_uri)
  }

  let redirect = move |Host(host): Host, uri: Uri| async move {
    match make_https(host, uri) {
      Ok(uri) => Ok(Redirect::permanent(&uri.to_string())),
      Err(e) => {
        debug!("Error while redirecting: {e:?}");
        Err(StatusCode::BAD_REQUEST)
      }
    }
  };

  // Change to match where your app is hosted
  let addr = SocketAddr::from(([0, 0, 0, 0], 80));
  let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
  axum::serve(listener, redirect.into_make_service()).await
}

pub async fn server_locally(app: Router, port: u16) -> Result<()> {
  let addr = SocketAddr::from(([127, 0, 0, 1], port));
  let listener = tokio::net::TcpListener::bind(&addr)
    .await
    .context("Creating listener")?;
  Ok(axum::serve(listener, app).await?)
}

pub fn mk_hostname_router(
  map: HashMap<String, Router>,
) -> BoxCloneService<Request<Body>, Response<Body>, Infallible> {
  BoxCloneService::new(
    (move |Host(hostname): Host, request: Request<Body>| async move {
      for (name, router) in map {
        if hostname == name {
          println!("serving {name}");
          return router.oneshot(request).await;
        }
      }

      Ok(StatusCode::NOT_FOUND.into_response())
    })
    .into_service(),
  )
}
