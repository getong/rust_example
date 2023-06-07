use tokio::sync::mpsc;
use tokio::time::{timeout, Duration};

async fn some_computation(input: u32) -> String {
    format!("the result of computation {}", input)
}

async fn sender_task(tx: mpsc::Sender<String>) {
    for i in 0..10 {
        let res = some_computation(i).await;
        tx.send(res).await.unwrap();
    }
}

async fn receiver_task(mut rx: mpsc::Receiver<String>) {
    loop {
        tokio::select! {
            // res = rx.clone().recv() => {
            //     if let Some(res) = res {
            //         println!("Received: {}", res);
            //     } else {
            //         break;
            //     }
            // },
            _ = timeout(Duration::from_secs(1), rx.recv()) => {
                println!("Timeout occurred");
                break;
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let (tx, rx) = mpsc::channel(100);

    let sender = tokio::spawn(sender_task(tx));
    let receiver = tokio::spawn(receiver_task(rx));

    tokio::try_join!(sender, receiver).unwrap();
}
