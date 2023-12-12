use actix_web::{web, App, HttpResponse, HttpServer};

pub async fn health_check() -> HttpResponse {
  HttpResponse::Ok().finish()
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
  HttpServer::new(|| App::new().route("/health_check", web::get().to(health_check)))
    .bind("127.0.0.1:8000")?
    .run()
    .await
}

#[cfg(test)]
mod tests {
  use crate::health_check;

  #[tokio::test]
  async fn health_check_succeeds() {
    let response = health_check().await;
    assert!(response.status().is_success())
  }
}
