use bytes::Bytes;
use http_body_util::{combinators::BoxBody, BodyExt, Empty, Full};
use hyper::{server::conn::http1, service::service_fn, Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use std::{collections::HashMap, convert::Infallible, net::SocketAddr};
use tokio::net::TcpListener;

// curl http://localhost:1337

static INDEX: &[u8] = b"<html><body><form action=\"post\" method=\"post\">Name: <input type=\"text\" name=\"name\"><br>Number: <input type=\"text\" name=\"number\"><br><input type=\"submit\"></body></html>";
static MISSING: &[u8] = b"Missing field";
static NOTNUMERIC: &[u8] = b"Number field is not numeric";

// Using service_fn, we can turn this function into a `Service`.
async fn param_example(
    req: Request<hyper::body::Incoming>,
) -> Result<Response<BoxBody<Bytes, Infallible>>, hyper::Error> {
    match (req.method(), req.uri().path()) {
        (&Method::GET, "/") | (&Method::GET, "/post") => Ok(Response::new(full(INDEX))),
        (&Method::POST, "/post") => {
            let b = req.collect().await?.to_bytes();

            let params = form_urlencoded::parse(b.as_ref())
                .into_owned()
                .collect::<HashMap<String, String>>();

            let name = if let Some(n) = params.get("name") {
                n
            } else {
                return Ok(Response::builder()
                    .status(StatusCode::UNPROCESSABLE_ENTITY)
                    .body(full(MISSING))
                    .unwrap());
            };
            let number = if let Some(n) = params.get("number") {
                if let Ok(v) = n.parse::<f64>() {
                    v
                } else {
                    return Ok(Response::builder()
                        .status(StatusCode::UNPROCESSABLE_ENTITY)
                        .body(full(NOTNUMERIC))
                        .unwrap());
                }
            } else {
                return Ok(Response::builder()
                    .status(StatusCode::UNPROCESSABLE_ENTITY)
                    .body(full(MISSING))
                    .unwrap());
            };

            let body = format!("Hello {}, your number is {}", name, number);
            Ok(Response::new(full(body)))
        }
        (&Method::GET, "/get") => {
            let query = if let Some(q) = req.uri().query() {
                q
            } else {
                return Ok(Response::builder()
                    .status(StatusCode::UNPROCESSABLE_ENTITY)
                    .body(full(MISSING))
                    .unwrap());
            };
            let params = form_urlencoded::parse(query.as_bytes())
                .into_owned()
                .collect::<HashMap<String, String>>();
            let page = if let Some(p) = params.get("page") {
                p
            } else {
                return Ok(Response::builder()
                    .status(StatusCode::UNPROCESSABLE_ENTITY)
                    .body(full(MISSING))
                    .unwrap());
            };
            let body = format!("You requested {}", page);
            Ok(Response::new(full(body)))
        }
        _ => Ok(Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(empty())
            .unwrap()),
    }
}

fn empty() -> BoxBody<Bytes, Infallible> {
    Empty::<Bytes>::new().boxed()
}

fn full<T: Into<Bytes>>(chunk: T) -> BoxBody<Bytes, Infallible> {
    Full::new(chunk.into()).boxed()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    pretty_env_logger::init();

    let addr: SocketAddr = ([127, 0, 0, 1], 1337).into();

    let listener = TcpListener::bind(addr).await?;
    println!("Listening on http://{}", addr);
    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);

        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .serve_connection(io, service_fn(param_example))
                .await
            {
                println!("Error serving connection: {:?}", err);
            }
        });
    }
}
