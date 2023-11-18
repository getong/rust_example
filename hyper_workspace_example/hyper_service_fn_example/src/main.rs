use hyper::service::service_fn;
use hyper::{Body, Request, Response, Server};
use std::convert::Infallible;
use std::net::SocketAddr;
use tower::make::Shared;

async fn handle(_req: Request<Body>) -> Result<Response<Body>, Infallible> {
    Ok(Response::new(Body::from("Hello World")))
}

#[tokio::main]
async fn main() {
    // Construct our SocketAddr to listen on...
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));

    // Shared is a MakeService that produces services by cloning an inner service...
    let make_service = Shared::new(service_fn(handle));

    // Then bind and serve...
    let server = Server::bind(&addr).serve(make_service);

    // And run forever...
    if let Err(e) = server.await {
        eprintln!("server error: {}", e);
    }
}
