use futures::StreamExt;
use rand::Rng;
use tokio::sync::mpsc;

#[derive(Debug)]
struct Work {
    request: String,
}

#[derive(Debug)]
struct Result {
    response: String,
}

async fn do_work(work: Work) -> Result {
    let rng = rand::thread_rng().gen_range(500..1500);
    tokio::time::sleep(std::time::Duration::from_millis(rng)).await;

    Result {
        response: format!("{}_processed", work.request),
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let (tx_work, rx_work) = mpsc::channel(100);

    let consumer = tokio::spawn(consumer(rx_work));

    for idx in 0..20 {
        tx_work
            .send(Work {
                request: format!("work_{}", idx),
            })
            .await?;
    }
    drop(tx_work);

    consumer.await?;

    Ok(())
}

async fn consumer(mut incoming: mpsc::Receiver<Work>) {
    let stream = async_stream::stream! {
        while let Some(item) = incoming.recv().await {
            yield do_work(item);
        }
    };

    let queue = stream.buffer_unordered(5);
    futures::pin_mut!(queue);

    while let Some(result) = queue.next().await {
        println!("{}_processed", result.response);
    }
}

// copy from https://willbaker.dev/posts/rust-networking-concurrency/
