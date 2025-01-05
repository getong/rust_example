use std::env;

use http_body_util::Empty;
use hyper::Request;
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
