use actix_protobuf::{ProtoBuf, ProtoBufResponseBuilder as _};
use actix_web::{middleware, web, App, HttpResponse, HttpServer, Result};
use prost::Message;

#[derive(Clone, PartialEq, Eq, Message)]
pub struct MyObj {
  #[prost(int32, tag = "1")]
  pub number: i32,

  #[prost(string, tag = "2")]
  pub name: String,
}

#[derive(Clone, PartialEq, Eq, Message)]
pub struct MyObj2 {
  #[prost(int32, tag = "1")]
  pub number: i32,

  #[prost(string, tag = "2")]
  pub name: String,
}

// Handler for MyObj type
async fn index_myobj(msg: ProtoBuf<MyObj>) -> Result<HttpResponse> {
  log::info!("Received MyObj: {:?}", msg.0);
  HttpResponse::Ok().protobuf(msg.0) // Send response
}

// Handler for MyObj2 type
async fn index_myobj2(msg: ProtoBuf<MyObj2>) -> Result<HttpResponse> {
  log::info!("Received MyObj2: {:?}", msg.0);
  HttpResponse::Ok().protobuf(msg.0) // Send response
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
  env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
  log::info!("starting HTTP server at http://localhost:8080");

  HttpServer::new(|| {
    App::new()
      .service(web::resource("/").route(web::post().to(index_myobj)))
      .service(web::resource("/myobj2").route(web::post().to(index_myobj2)))
      .wrap(middleware::Logger::default())
  })
  .workers(3)
  .bind(("127.0.0.1", 8080))?
  .run()
  .await
}
