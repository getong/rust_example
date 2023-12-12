use tonic::{
  transport::{Certificate, Server, ServerTlsConfig},
  Request, Response, Status,
};

use hello_world::greeter_server::{Greeter, GreeterServer};
use hello_world::{HelloReply, HelloRequest};

pub mod hello_world {
  tonic::include_proto!("helloworld");
}

#[derive(Default)]
pub struct MyGreeter {}

#[tonic::async_trait]
impl Greeter for MyGreeter {
  async fn say_hello(
    &self,
    request: Request<HelloRequest>,
  ) -> Result<Response<HelloReply>, Status> {
    println!("Got a request from {:?}", request.remote_addr());

    let reply = hello_world::HelloReply {
      message: format!("Hello {}!", request.into_inner().name),
    };
    Ok(Response::new(reply))
  }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let addr = "[::1]:50051".parse().unwrap();
  let greeter = MyGreeter::default();

  println!("GreeterServer listening on {}", addr);

  let cert_pem = std::fs::read_to_string("client.pem").expect("cert path wrong");
  let key_pem = std::fs::read_to_string("client.key").expect("key path wrong");
  // let cert = include_bytes!("../client.pem");
  // let key = include_bytes!("../client.key");
  let identity = tonic::transport::Identity::from_pem(cert_pem.as_bytes(), key_pem.as_bytes());

  // let ca_cert = include_bytes!("../ca_cert.pem");
  let ca_cert = std::fs::read_to_string("ca_cert.pem").expect("key path wrong");

  let cert = Certificate::from_pem(ca_cert.as_bytes());
  let server_tls_config = ServerTlsConfig::new()
    .client_ca_root(cert)
    .client_auth_optional(false)
    .identity(identity);

  Server::builder()
    .tls_config(server_tls_config)
    .expect("wrong certifacate file")
    .add_service(GreeterServer::new(greeter))
    .serve(addr)
    .await?;

  Ok(())
}
