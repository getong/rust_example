use bytes::Bytes;
use http_body_util::BodyExt;
use http_body_util::Empty;
use hyper::Uri;
use hyper_tls::HttpsConnector;
use hyper_util::{client::legacy::Client, rt::TokioExecutor};
use tokio::sync::{mpsc, oneshot};

type FutureResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync + 'static>>;

enum Request {
  Get(Uri, oneshot::Sender<Bytes>),
  Quit,
}

async fn https_layer(mut rx_chan: mpsc::Receiver<Request>) -> FutureResult<()> {
  let https = HttpsConnector::new();
  let client = Client::builder(TokioExecutor::new()).build::<_, Empty<Bytes>>(https);

  while let Some(request) = rx_chan.recv().await {
    match request {
      Request::Get(url, response) => {
        let resp = client.get(url).await?;

        let reversed_body = resp.collect().await?.to_bytes();
        // let body = String::from_utf8(reversed_body)?;

        _ = match response.send(reversed_body) {
          Ok(()) => Ok(()),
          _ => Err("send_bytes_error".to_string()),
        };
      }
      Request::Quit => {
        break;
      }
    }
  }

  Ok(())
}

async fn get_html(url: Uri, tx_chan: mpsc::Sender<Request>) -> FutureResult<()> {
  let (resp_tx, resp_rx) = oneshot::channel();

  tx_chan.send(Request::Get(url.clone(), resp_tx)).await?;

  let res = resp_rx.await?;

  println!("Response for {}: {:?}", url, res);

  Ok(())
}

#[tokio::main]
async fn main() -> FutureResult<()> {
  let (http_tx, http_rx) = mpsc::channel::<Request>(100);

  let join_handle = tokio::spawn(https_layer(http_rx));

  get_html(Uri::from_static("https://www.baidu.com"), http_tx.clone()).await?;
  get_html(Uri::from_static("https://www.163.com"), http_tx.clone()).await?;

  // Send a Quit request to stop the https_layer task
  http_tx.send(Request::Quit).await?;

  // Wait for the https_layer task to complete
  let _ = join_handle.await?;

  Ok(())
}
