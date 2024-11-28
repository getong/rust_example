use std::net::TcpListener;

use actix_web::{dev::Server, web, App, HttpResponse, HttpServer};

pub async fn health_check() -> HttpResponse {
  HttpResponse::Ok().finish()
}

pub fn run(listener: TcpListener) -> Result<Server, std::io::Error> {
  let server = HttpServer::new(|| App::new().route("/health_check", web::get().to(health_check)))
    .listen(listener)?
    .run();
  // No .await here!
  Ok(server)
}
