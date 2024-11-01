use axum::{
  middleware,
  routing::{get, post},
  Router,
};
use tokio::net::TcpListener;

mod auth;
mod services;

use crate::{
  auth::{authorize, sign_in},
  services::hello,
};

#[tokio::main]
async fn main() {
  let listener = TcpListener::bind("127.0.0.1:8080")
    .await
    .expect("Unable to conne to connect to the server");

  println!("Listening on {}", listener.local_addr().unwrap());

  let app = Router::new().route("/signin", post(sign_in)).route(
    "/protected/",
    get(hello).layer(middleware::from_fn(authorize)),
  );

  axum::serve(listener, app)
    .await
    .expect("Error serving application");
}
