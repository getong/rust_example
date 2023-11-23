use actix_web_integration_test_example::run;
use std::net::TcpListener;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    // println!("Hello, world!");
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind random port");
    // We retrieve the port assigned to us by the OS
    let port = listener.local_addr().unwrap().port();
    println!("listen to http://127.0.0.1:{}/health_check", port);
    run(listener)?.await
}
