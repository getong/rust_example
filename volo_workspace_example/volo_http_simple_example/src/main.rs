use std::{convert::Infallible, net::SocketAddr, sync::Arc, time::Duration};

use async_stream::stream;
use bytes::Bytes;
use faststr::FastStr;
use serde::{Deserialize, Serialize};
use tokio_stream::StreamExt;
use volo::{context::Context, service::service_fn};
use volo_http::{
  Address,
  body::Body,
  context::ServerContext,
  http::{Method, StatusCode, Uri, header},
  hyper::body::{Body as HttpBody, Frame},
  request::Request,
  response::Response,
  server::{
    IntoResponse, Server,
    extract::{Form, Json, MaybeInvalid, Query},
    layer::{FilterLayer, TimeoutLayer},
    middleware::{self, Next},
    param::PathParamsMap,
    route::{Router, get, get_service, post},
  },
  utils::{
    Extension,
    cookie::{self, CookieJar},
  },
};

async fn hello() -> &'static str {
  "hello, world\n"
}

#[derive(Serialize, Deserialize, Debug)]
struct Person {
  name: String,
  age: u8,
  phones: Vec<String>,
}

async fn json_get() -> Json<Person> {
  Json(Person {
    name: "Foo".to_string(),
    age: 25,
    phones: vec!["Bar".to_string(), "114514".to_string()],
  })
}

async fn json_post(Json(request): Json<Person>) -> String {
  let first_phone = request
    .phones
    .first()
    .map(|p| p.as_str())
    .unwrap_or("no number");
  format!(
    "{} is {} years old, {}'s first phone number is `{}`\n",
    request.name, request.age, request.name, first_phone
  )
}

async fn json_post_with_check(request: Option<Json<Person>>) -> Result<String, StatusCode> {
  let request = match request {
    Some(Json(req)) => req,
    None => {
      return Err(StatusCode::BAD_REQUEST);
    }
  };
  let first_phone = request
    .phones
    .first()
    .map(|p| p.as_str())
    .unwrap_or("no number");
  Ok(format!(
    "{} is {} years old, {}'s first phone number is `{}`\n",
    request.name, request.age, request.name, first_phone
  ))
}

#[derive(Deserialize, Debug)]
struct Login {
  username: String,
  password: String,
}

fn process_login(info: Login) -> Result<String, StatusCode> {
  if info.username == "admin" && info.password == "password" {
    Ok("Login Success!".to_string())
  } else {
    Err(StatusCode::IM_A_TEAPOT)
  }
}

async fn get_with_query(Query(info): Query<Login>) -> Result<String, StatusCode> {
  process_login(info)
}

async fn post_with_form(Form(info): Form<Login>) -> Result<String, StatusCode> {
  process_login(info)
}

async fn get_and_post(
  u: Uri,
  m: Method,
  data: MaybeInvalid<FastStr>,
) -> Result<String, (StatusCode, &'static str)> {
  let msg = unsafe { data.assume_valid() };
  match m {
    Method::GET => Err((StatusCode::BAD_REQUEST, "Try POST something\n")),
    Method::POST => Ok(format!("{m} {u}\n\n{msg}\n")),
    _ => unreachable!(),
  }
}

async fn timeout_test() {
  tokio::time::sleep(Duration::from_secs(10)).await
}

async fn echo(params: PathParamsMap) -> Result<FastStr, StatusCode> {
  if let Some(echo) = params.get("echo") {
    return Ok(echo.to_owned());
  }
  Err(StatusCode::BAD_REQUEST)
}

async fn conn_show(conn: Address) -> String {
  format!("{conn:?}\n")
}

async fn stream_test() -> Body {
  // build a `Vec<u8>` by a string
  let resp = "Hello, this is a stream.\n".as_bytes().iter().copied();
  // convert each byte to a `Bytes`
  let stream = stream! {
      for ch in resp.into_iter() {
          yield Bytes::from(vec![ch]);
      }
  };
  // map `Stream<Item = Bytes>` to `Steram<Item = Result<Frame<Bytes>, BoxError>>`
  Body::from_stream(stream.map(|b| Ok(Frame::data(b))))
}

async fn box_body_test() -> Body {
  let body = stream_test().await;
  Body::from_body(body)
}

struct State {
  foo: String,
  bar: usize,
}

async fn extension(Extension(state): Extension<Arc<State>>) -> String {
  format!("State {{ foo: {}, bar: {} }}\n", state.foo, state.bar)
}

async fn service_fn_test(cx: &mut ServerContext, req: Request) -> Result<Response, Infallible> {
  Ok(format!("cx: {cx:?}, req: {req:?}").into_response())
}

fn timeout_handler(cx: &ServerContext) -> StatusCode {
  tracing::info!(
    "Request timed out, peer: {:?}",
    cx.rpc_info().caller().address()
  );
  StatusCode::INTERNAL_SERVER_ERROR
}

fn index_router() -> Router {
  // curl http://127.0.0.1:8080/
  Router::new().route("/", get(hello))
}

fn user_json_router() -> Router {
  Router::new()
    // curl http://localhost:8080/user/json_get
    .route("/user/json_get", get(json_get))
    // curl http://localhost:8080/user/json_post \
    //     -X POST \
    //     -H "Content-Type: application/json" \
    //     -d '{"name":"Foo", "age": 25, "phones":["Bar", "114514"]}'
    .route("/user/json_post", post(json_post))
    // curl http://localhost:8080/user/json_post_with_check \
    //     -X POST \
    //     -H "Content-Type: application/json" \
    //     -d '{"name":"Foo", "age": -1, "phones":["Bar", "114514"]}'
    //
    // Note that this is an invalid json
    .route("/user/json_post_with_check", post(json_post_with_check))
}

fn user_form_router() -> Router {
  Router::new().route(
    "/user/login",
    // curl "http://localhost:8080/user/login?username=admin&password=admin"
    // curl "http://localhost:8080/user/login?username=admin&password=password"
    get(get_with_query)
      // curl http://localhost:8080/user/login \
      //     -X POST \
      //     -d 'username=admin&password=admin'
      // curl http://localhost:8080/user/login \
      //     -X POST \
      //     -d 'username=admin&password=password'
      .post(post_with_form),
  )
}

fn test_router() -> Router {
  Router::new()
    // curl http://127.0.0.1:8080/test/extract
    // curl http://127.0.0.1:8080/test/extract -X POST -d "114514"
    .route("/test/extract", get(get_and_post).post(get_and_post))
    // curl http://127.0.0.1:8080/test/timeout
    .route(
      "/test/timeout",
      get(timeout_test).layer(TimeoutLayer::new(Duration::from_secs(1), timeout_handler)),
    )
    // curl -v http://127.0.0.1:8080/test/param/114514
    .route("/test/param/{echo}", get(echo))
    // curl http://127.0.0.1:8080/test/conn_show
    .route("/test/conn_show", get(conn_show))
    // curl http://127.0.0.1:8080/test/extension
    .route("/test/extension", get(extension))
    // curl http://127.0.0.1:8080/test/service_fn
    .route("/test/service_fn", get_service(service_fn(service_fn_test)))
    // curl -v http://127.0.0.1:8080/test/stream
    .route("/test/stream", get(stream_test))
    // curl -v http://127.0.0.1:8080/test/body
    .route("/test/body", get(box_body_test))
    // curl -v http://127.0.0.1:8080/test/anyaddr?reject_me
    .layer(FilterLayer::new(|uri: Uri| async move {
      if uri.query().is_some() && uri.query().unwrap() == "reject_me" {
        Err(StatusCode::INTERNAL_SERVER_ERROR)
      } else {
        Ok(())
      }
    }))
}

// You can use the following commands for testing cookies
//
// ```bash
// # create a cookie jar for `curl`
// TMPFILE=$(mktemp --tmpdir cookie_jar.XXXXXX)
//
// # access it for more than one times!
// curl -v http://127.0.0.1:8080/ -b $TMPFILE -c $TMPFILE
// curl -v http://127.0.0.1:8080/ -b $TMPFILE -c $TMPFILE
// # ......
// ```
async fn tracing_from_fn(
  uri: Uri,
  peer: Address,
  cookie_jar: CookieJar,
  cx: &mut ServerContext,
  req: Request,
  next: Next,
) -> Response {
  tracing::info!("{:?}", *cookie_jar);
  let count = cookie_jar.get("count").map_or(0usize, |val| {
    val.value().to_string().parse().unwrap_or(0usize)
  });
  let start = std::time::Instant::now();
  let resp = next.run(cx, req).await;
  let elapsed = start.elapsed();

  tracing::info!("seq: {count}: {peer} request {uri}, cost {elapsed:?}");

  (
    (
      header::SET_COOKIE,
      cookie::Cookie::build(("count", format!("{}", count + 1)))
        .path("/")
        .max_age(cookie::Duration::days(1))
        .build()
        .to_string(),
    ),
    resp,
  )
    .into_response()
}

async fn headers_map_response(response: Response) -> impl IntoResponse {
  (
    [
      ("Access-Control-Allow-Origin", "*"),
      ("Access-Control-Allow-Headers", "*"),
      ("Access-Control-Allow-Method", "*"),
    ],
    response,
  )
}

async fn trace_stats(cx: &mut ServerContext, req: Request, next: Next) -> Response {
  let start = std::time::Instant::now();
  let req_size = req.body().size_hint();
  let resp = next.run(cx, req).await.into_response();
  tracing::info!(
    "request completed in {:?}, req size: {:?}, resp size: {:?}, resp status: {}",
    start.elapsed(),
    req_size,
    resp.body().size_hint(),
    resp.status(),
  );
  resp
}

#[volo::main]
async fn main() {
  let subscriber = tracing_subscriber::FmtSubscriber::builder()
    .with_max_level(tracing::Level::TRACE)
    .finish();
  tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

  let app = Router::new()
    .merge(index_router())
    .merge(user_json_router())
    .merge(user_form_router())
    .merge(test_router())
    .layer(Extension(Arc::new(State {
      foo: "Foo".to_string(),
      bar: 114514,
    })))
    .layer(middleware::from_fn(tracing_from_fn))
    .layer(middleware::map_response(headers_map_response))
    .layer(TimeoutLayer::new(
      Duration::from_secs(5),
      |_: &ServerContext| StatusCode::INTERNAL_SERVER_ERROR,
    ));

  let addr: SocketAddr = "[::]:8080".parse().unwrap();
  let addr = volo::net::Address::from(addr);

  println!("Listening on {addr}");

  Server::new(app)
    .layer(middleware::from_fn(trace_stats))
    .run(addr)
    .await
    .unwrap();
}

#[cfg(test)]
mod tests {
  use volo::Service;
  use volo_http::body::BodyConversion;

  use super::*;

  #[tokio::test]
  async fn migrated_routes_handle_requests() {
    let router = test_router();
    for (path, expected) in [
      ("/test/param/114514", "114514"),
      ("/test/stream", "Hello, this is a stream.\n"),
      ("/test/body", "Hello, this is a stream.\n"),
    ] {
      let mut cx = ServerContext::new("127.0.0.1:12345".parse::<SocketAddr>().unwrap().into());
      let req = Request::builder().uri(path).body(Body::empty()).unwrap();
      let resp = router.call(&mut cx, req).await.unwrap();
      assert_eq!(resp.status(), StatusCode::OK);
      assert_eq!(resp.into_body().into_string().await.unwrap(), expected);
    }
  }

  #[tokio::test]
  async fn migrated_service_route_and_timeout_work() {
    let router = test_router();
    for (path, expected) in [
      ("/test/service_fn", StatusCode::OK),
      ("/test/timeout", StatusCode::INTERNAL_SERVER_ERROR),
    ] {
      let mut cx = ServerContext::new("127.0.0.1:12345".parse::<SocketAddr>().unwrap().into());
      let req = Request::builder().uri(path).body(Body::empty()).unwrap();
      let resp = router.call(&mut cx, req).await.unwrap();
      assert_eq!(resp.status(), expected);
    }
  }
}
