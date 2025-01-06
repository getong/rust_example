use std::env;

use http_body_util::{Empty, Full};
use hyper::{body::Bytes, Request, Uri};
use hyper_util::client::legacy::Client as HyperClient;
use tower_service::Service;

// cargo run -- http://www.baidu.com --header "header=User-Agent: MyClient"

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  // Capture the URL from the first argument
  let url = match env::args().nth(1) {
    Some(url) => url,
    None => {
      eprintln!("Usage: client <url> [extra_args]");
      return Ok(());
    }
  };

  // Capture extra arguments, if any
  let extra_args: Vec<String> = env::args().skip(2).collect();

  // Log extra arguments (for demonstration)
  if !extra_args.is_empty() {
    eprintln!("Extra Arguments: {:?}", extra_args);
  }

  // Parse the URL
  let url = url.parse::<hyper::Uri>()?;
  if url.scheme_str() != Some("http") {
    eprintln!("This example only works with 'http' URLs.");
    return Ok(());
  }
  println!("request with empty bytes\n\n");
  _ = request_empty_body(url.clone(), extra_args.clone()).await?;

  println!("request with full bytes\n\n");
  _ = request_full_body(url, extra_args).await?;

  Ok(())
}

async fn request_empty_body(
  url: Uri,
  extra_args: Vec<String>,
) -> Result<(), Box<dyn std::error::Error>> {
  // Set up the client
  let mut client = HyperClient::builder(hyper_util::rt::TokioExecutor::new()).build_http();

  // Create the HTTP request
  let mut req = Request::builder()
    .uri(url)
    .body(Empty::<bytes::Bytes>::new())?;

  // If extra_args contains a custom header, add it to the request
  if let Some(custom_header) = extra_args.iter().find(|&arg| arg.starts_with("header=")) {
    let header_value = custom_header.splitn(2, '=').nth(1).unwrap_or("");
    let header_name = "X-Custom-Header"; // Example header
    req.headers_mut().insert(header_name, header_value.parse()?);
  }
  eprintln!("req headers is {:?}", req.headers());

  // Send the request
  let resp = client.request(req.clone()).await?;
  eprintln!("{:?} {:?}", resp.version(), resp.status());
  eprintln!("{:#?}", resp.headers());
  // Optionally process the extra_args (e.g., if they specify query params or other options)
  let result = client.call(req).await;
  eprintln!("result : {:?}", result);
  Ok(())
}

async fn request_full_body(
  url: Uri,
  extra_args: Vec<String>,
) -> Result<(), Box<dyn std::error::Error>> {
  let mut client = HyperClient::builder(hyper_util::rt::TokioExecutor::new()).build_http();

  // Example payload for Full<Bytes> request
  let payload = r#"
        {
            "key": "value",
            "message": "Hello, world!"
        }
    "#;

  // Convert the payload to bytes
  let bytes = Bytes::from(payload);

  // Explicitly define the type for `req_with_body` as
  // `hyper::Request<http_body_util::Full<bytes::Bytes>>`
  let mut req_with_body: hyper::Request<http_body_util::Full<bytes::Bytes>> = Request::builder()
    .uri(url)
    .header("Content-Type", "application/json")
    .body(Full::from(bytes))?;

  // If extra_args contains a custom header, add it to the request (same as before)
  if let Some(custom_header) = extra_args.iter().find(|&arg| arg.starts_with("header=")) {
    let header_value = custom_header.splitn(2, '=').nth(1).unwrap_or("");
    let header_name = "X-Custom-Header"; // Example header
    req_with_body
      .headers_mut()
      .insert(header_name, header_value.parse()?);
  }
  eprintln!("req headers (Full<Bytes>) is {:?}", req_with_body.headers());

  // Send the request with Full<Bytes> body
  let resp_with_body = client.request(req_with_body.clone()).await?;
  eprintln!(
    "{:?} {:?}",
    resp_with_body.version(),
    resp_with_body.status()
  );
  eprintln!("{:#?}", resp_with_body.headers());

  // Optionally process the extra_args (e.g., if they specify query params or other options)
  let result = client.call(req_with_body).await;
  eprintln!("result with Full<Bytes>: {:?}", result);

  Ok(())
}
