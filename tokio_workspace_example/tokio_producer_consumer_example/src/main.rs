use std::sync::Arc;

use rand::Rng;
use tokio::sync::{mpsc, Semaphore};

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
    let (tx_work, rx_work) = mpsc::channel(1);
    let (tx_result, mut rx_result) = mpsc::channel(1);

    tokio::spawn(worker(rx_work, tx_result));
    tokio::spawn(producer(tx_work));

    while let Some(result) = rx_result.recv().await {
        println!("{}", result.response);
    }

    Ok(())
}

async fn producer(tx_work: mpsc::Sender<Work>) {
    for idx in 0..20 {
        tx_work
            .send(Work {
                request: format!("work_{}", idx),
            })
            .await
            .unwrap();
    }
}

async fn worker(
    mut rx_work: mpsc::Receiver<Work>,
    tx_result: mpsc::Sender<Result>,
) -> anyhow::Result<()> {
    let semaphore = Arc::new(Semaphore::new(5));

    while let Some(work) = rx_work.recv().await {
        let permit = semaphore.clone().acquire_owned().await?;
        let tx = tx_result.clone();
        tokio::spawn(async move {
            tx.send(do_work(work).await).await.unwrap();
            drop(permit)
        });
    }

    Ok(())
}
