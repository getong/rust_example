use bytes::Bytes;
use reqwest::Error;
use tokio_stream::StreamExt; // To use stream combinators like `next`

pub type Result<T, E = Error> = std::result::Result<T, E>;

pub async fn fetch_url(url: &str) -> Result<impl tokio_stream::Stream<Item = Result<Bytes>>> {
    let response = reqwest::get(url).await?;
    Ok(response.bytes_stream())
}

#[tokio::main]
async fn main() {
    let mut byte_list = vec![];
    if let Ok(mut stream) = fetch_url("https://www.baidu.com").await {
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(bytes) => {
                    println!(
                        "Received bytes: {:?}",
                        String::from_utf8_lossy(&bytes).into_owned()
                    );
                    byte_list.extend_from_slice(&bytes);
                }
                e => eprintln!("Error while streaming: {:?}", e),
            }
        }
    }

    println!(
        "total {}",
        String::from_utf8_lossy(&Bytes::from(byte_list)).into_owned()
    )
}
