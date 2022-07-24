use hyper::body;
use hyper::{client::Client, Body, Uri};
use hyper_tls::HttpsConnector;
use tokio::sync::{mpsc, oneshot};

type FutureResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync + 'static>>;

async fn https_layer(
    mut rx_chan: mpsc::Receiver<(&'static str, oneshot::Sender<String>)>,
) -> FutureResult<()> {
    let https = HttpsConnector::new();
    let client = Client::builder().build::<_, Body>(https);

    while let Some((url, response)) = rx_chan.recv().await {
        let resp = client.get(Uri::from_static(url)).await?;

        let body_bytes = body::to_bytes(resp.into_body()).await?;
        let body = String::from_utf8(body_bytes.to_vec())?;

        response.send(body)?;
    }

    Ok(())
}

async fn get_html(
    url: &'static str,
    tx_chan: &mut mpsc::Sender<(&'static str, oneshot::Sender<String>)>,
) -> FutureResult<()> {
    let (resp_tx, resp_rx) = oneshot::channel();

    tx_chan.send((url, resp_tx)).await?;

    let res = resp_rx.await?;

    println!("previous value = {}", res);

    Ok(())
}

#[tokio::main]
async fn main() -> FutureResult<()> {
    let (mut http_tx, http_rx) = mpsc::channel::<(&'static str, oneshot::Sender<String>)>(100);

    let join_handle = tokio::spawn(https_layer(http_rx));

    get_html("https://stackoverflow.com", &mut http_tx).await?;
    get_html("https://google.com", &mut http_tx).await?;

    let _ = join_handle.await?;
    Ok(())
}
