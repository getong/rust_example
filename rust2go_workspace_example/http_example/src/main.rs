mod http;

use http::{HttpFetchCall, HttpFetchCallImpl, HttpFetchRequest};

fn main() {
  let response = HttpFetchCallImpl::fetch(&HttpFetchRequest {
    url: "https://gobyexample.com".to_string(),
    max_lines: 5,
  });

  if !response.error.is_empty() {
    panic!("request failed: {}", response.error);
  }

  println!("Response status: {}", response.status);
  for line in response.lines {
    println!("{line}");
  }
}
