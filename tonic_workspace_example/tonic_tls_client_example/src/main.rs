use std::time::Duration;

use hello_world::{greeter_client::GreeterClient, HelloRequest};
use tonic::transport::{Certificate, Channel, ClientTlsConfig};
// use tonic::Request;

pub mod hello_world {
  tonic::include_proto!("helloworld");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let cert = std::fs::read_to_string("ca_cert.pem")?;

  let cert_pem = std::fs::read_to_string("client.pem").expect("cert path wrong");
  let key_pem = std::fs::read_to_string("client.key").expect("key path wrong");
  // let cert = include_bytes!("../client.pem");
  // let key = include_bytes!("../client.key");
  let identity = tonic::transport::Identity::from_pem(cert_pem.as_bytes(), key_pem.as_bytes());

  let channel = Channel::from_static("https://[::1]:50051")
    .tls_config(
      ClientTlsConfig::new()
        .identity(identity)
        .ca_certificate(Certificate::from_pem(&cert))
        .domain_name("localhost".to_string()),
    )?
    .timeout(Duration::from_secs(5))
    .rate_limit(5, Duration::from_secs(1))
    .concurrency_limit(256)
    .connect()
    .await?;

  let mut client = GreeterClient::new(channel);

  let request = tonic::Request::new(HelloRequest {
    name: "Tonic".into(),
  });

  let response = client.say_hello(request).await?;

  println!("RESPONSE={:?}", response);

  let request = tonic::Request::new(HelloRequest {
    name: "Tonic".into(),
  });

  let response2 = client.say_hi(request).await?;

  println!("RESPONSE2={:?}", response2);

  Ok(())
}
