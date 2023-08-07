use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::time::{timeout, Duration};

type MpscInnerTy<T, U> = (T, oneshot::Sender<U>);

#[tokio::main]
async fn main() {
    // Create an unbounded mpsc channel for MpscInnerTy
    let (tx, mut rx) = mpsc::unbounded_channel::<MpscInnerTy<String, String>>();

    // Spawn a Tokio task to send a value through the channel
    tokio::spawn(async move {
        let (send, recv) = oneshot::channel();
        let mpsc_inner = ("Hello from the sender!".to_string(), send);

        // Send the MpscInnerTy through the unbounded mpsc channel
        tx.send(mpsc_inner).unwrap();

        // Simulate some work in the sender task
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        // // Complete the oneshot channel
        // let msg = recv.try_recv();
        // println!("msg: {:?}", msg);
        let timeout_duration = Duration::from_secs(2);
        match timeout(timeout_duration, recv).await {
            Ok(result) => match result {
                Ok(value) => {
                    println!("Received: {}", value);
                }
                Err(e1) => {
                    println!(
                        "Timed out: Value not received within the timeout, e:{:?}",
                        e1
                    );
                }
            },
            Err(e) => {
                println!("Error receiving value, the e: {:?}", e);
            }
        }
    });

    // Receive the MpscInnerTy from the unbounded mpsc channel
    if let Some((message, oneshot_sender)) = rx.recv().await {
        // Spawn a Tokio task to complete the oneshot channel
        println!("Received message: {}", message);
        tokio::spawn(async move {
            let _ = oneshot_sender.send(message);
        });
    }

    // Wait for both tasks to complete
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
}
