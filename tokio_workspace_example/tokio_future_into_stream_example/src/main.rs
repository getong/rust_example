use futures::FutureExt;
use tokio_stream::Stream;
use tokio_stream::StreamExt;

fn outer() -> impl Stream<Item = i32> {
    inner().into_stream()
}

async fn inner() -> i32 {
    42
}

#[tokio::main]
async fn main() {
    let out_stream = outer();
    tokio::pin!(out_stream);
    while let Some(i) = out_stream.next().await {
        println!("i: {:?}", i);
    }
}
