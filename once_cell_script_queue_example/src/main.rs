use once_cell::sync::OnceCell;
use tokio::sync::{mpsc, oneshot};
use tokio::time::{sleep, Duration};

#[derive(Debug)]
pub enum Packet {
    Exec(usize, String, oneshot::Sender<Result<String, String>>),
    Close,
}

pub static SCRIPT_QUEUE: OnceCell<mpsc::Sender<Packet>> = OnceCell::new();

#[tokio::main]
async fn main() {
    // println!("Hello, world!");
    let (tx, mut rx) = tokio::sync::mpsc::channel(64);
    SCRIPT_QUEUE.set(tx.clone()).unwrap();

    let handler = tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Some(Packet::Exec(id, source, sender)) => {
                    println!("id:{:?}, source:{:?}", id, source);
                    let out = Ok("hello".to_string());
                    if sender.send(out).is_err() {
                        println!("Error sending result of script execution:\n{}", source);
                    }
                }
                other => {
                    println!("other:{:?}", other);
                    // break;
                }
            }
        }
    });

    let (tx2, rx2) = oneshot::channel();
    _ = SCRIPT_QUEUE
        .get()
        .unwrap()
        .send(Packet::Exec(1, "abc".to_string(), tx2))
        .await;

    sleep(Duration::from_millis(1000)).await;
    let result = rx2.await;
    println!("result:{:?}", result);

    println!("handler status: {:?}", handler.is_finished());
    loop {}
}
