use axum::Server;
use axum_streams::*;
use futures::prelude::*;
use reqwest_streams::*;
use std::net::TcpListener;
use tower::make::Shared;

#[derive(Clone, prost::Message)]
struct MyTestStructure {
    #[prost(string, tag = "1")]
    some_test_field: String,
}

fn source_test_stream() -> impl Stream<Item = MyTestStructure> {
    // Simulating a stream with a plain vector
    stream::iter(vec![
        MyTestStructure {
            some_test_field: "TestValue".to_string()
        };
        3
    ])
}

async fn test_proto_buf() -> impl axum::response::IntoResponse {
    StreamBodyAs::protobuf(source_test_stream())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Could not bind ephemeral socket");
    let addr = listener.local_addr().unwrap();
    println!("Listening on {}", addr);

    let svc = axum::Router::new().route("/protobuf", axum::routing::get(test_proto_buf));

    tokio::spawn(async move {
        let server = Server::from_tcp(listener).unwrap().serve(Shared::new(svc));
        server.await.expect("server error");
    });

    println!("Requesting protobuf");

    let resp1 = reqwest::get(format!("http://{}/protobuf", addr))
        .await?
        .protobuf_stream::<MyTestStructure>(1024);

    let items1: Vec<MyTestStructure> = resp1.try_collect().await?;

    println!("{:#?}", items1);

    Ok(())
}
