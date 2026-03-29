#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
compile_error!("This example currently supports only Linux, macOS, and Windows.");

#[cfg(target_os = "linux")]
const TLS_BACKEND: &str = "default-tls";

#[cfg(any(target_os = "macos", target_os = "windows"))]
const TLS_BACKEND: &str = "native-tls";

#[cfg(target_os = "linux")]
fn build_client() -> Result<reqwest::Client, reqwest::Error> {
  reqwest::Client::builder()
    .user_agent("reqwest-tls-example/0.1.0")
    .build()
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn build_client() -> Result<reqwest::Client, reqwest::Error> {
  reqwest::Client::builder()
    .use_native_tls()
    .user_agent("reqwest-tls-example/0.1.0")
    .build()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let client = build_client()?;
  let resp = client.get("https://httpbin.org/get").send().await?;

  println!(
    "os={}, tls_backend={}, status={}",
    std::env::consts::OS,
    TLS_BACKEND,
    resp.status()
  );

  Ok(())
}
