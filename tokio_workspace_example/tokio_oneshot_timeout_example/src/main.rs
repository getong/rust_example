use tokio::sync::oneshot;
use tokio::time::{timeout, Duration};

#[tokio::main]
async fn main() {
    // Create a oneshot channel
    let (tx, rx) = oneshot::channel();

    // Spawn a Tokio task to send a value through the channel after a delay
    tokio::spawn(async move {
        let message = "Hello from the other side!";
        tokio::time::sleep(Duration::from_secs(3)).await; // Simulate delay
        let _ = tx.send(message);
    });

    // Use timeout to receive the value with a timeout
    let timeout_duration = Duration::from_secs(2);
    match timeout(timeout_duration, rx).await {
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
}
