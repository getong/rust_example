use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::{Request, Uri};
use hyper_tls::HttpsConnector;
use hyper_util::{client::legacy::Client, rt::TokioExecutor};
use tokio::sync::{mpsc, oneshot};

type FutureResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync + 'static>>;

enum LocalRequest {
  Get(Uri, String, oneshot::Sender<Bytes>),
  Quit,
}

async fn https_layer(mut rx_chan: mpsc::Receiver<LocalRequest>) -> FutureResult<()> {
  let https = HttpsConnector::new();
  let client = Client::builder(TokioExecutor::new()).build::<_, Full<Bytes>>(https);

  while let Some(request) = rx_chan.recv().await {
    match request {
      LocalRequest::Get(url, body_string, response) => {
        let req = Request::builder()
          .uri(url)
          .body(Full::new(Bytes::from(body_string)))?;

        let resp = client.request(req).await?;

        let reversed_body = resp.collect().await?.to_bytes();

        _ = match response.send(reversed_body) {
          Ok(()) => Ok(()),
          _ => Err("send_bytes_error".to_string()),
        };
      }
      LocalRequest::Quit => {
        break;
      }
    }
  }

  Ok(())
}

async fn get_html(
  url: Uri,
  body_str: String,
  tx_chan: mpsc::Sender<LocalRequest>,
) -> FutureResult<()> {
  let (resp_tx, resp_rx) = oneshot::channel();

  tx_chan
    .send(LocalRequest::Get(url.clone(), body_str, resp_tx))
    .await?;

  let res = resp_rx.await?;

  println!("Response for {}: {:?}", url, res);

  Ok(())
}

#[tokio::main]
async fn main() -> FutureResult<()> {
  let (http_tx, http_rx) = mpsc::channel::<LocalRequest>(100);

  let join_handle = tokio::spawn(https_layer(http_rx));

  get_html(
    Uri::from_static("https://www.baidu.com"),
    "hello".to_string(),
    http_tx.clone(),
  )
  .await?;
  get_html(
    Uri::from_static("https://www.163.com"),
    "world".to_string(),
    http_tx.clone(),
  )
  .await?;

  // Send a Quit request to stop the https_layer task
  http_tx.send(LocalRequest::Quit).await?;

  // Wait for the https_layer task to complete
  let _ = join_handle.await?;

  Ok(())
}
