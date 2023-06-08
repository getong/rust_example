use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};

async fn receive(
    mut reader: tokio::net::tcp::OwnedReadHalf,
) -> (Vec<u8>, tokio::net::tcp::OwnedReadHalf) {
    let mut size = [0; 4];
    reader.read_exact(&mut size).await.unwrap();
    let size = u32::from_be_bytes(size) as usize;
    let mut buffer = vec![0; size];
    reader.read_exact(&mut buffer).await.unwrap();
    (buffer, reader)
}

async fn receive_wrapper(
    reader: tokio::net::tcp::OwnedReadHalf,
) -> (Vec<u8>, tokio::net::tcp::OwnedReadHalf) {
    receive(reader).await
}

#[tokio::main]
async fn main() {
    select_arc_example().await;

    let (_sender, mut receiver) = tokio::sync::mpsc::channel::<String>(32);
    // Send the sender to other threads

    let stream =
        tokio::net::TcpStream::connect(SocketAddr::new("127.0.0.1".parse().unwrap(), 7530))
            .await
            .unwrap();
    let (reader, mut writer) = stream.into_split();
    let mut receive_operation = Box::pin(receive_wrapper(reader));
    loop {
        tokio::select! {
            _ = &mut receive_operation => {
                let (buffer, next_reader) = receive_operation.await;
                // Parse the buffer here
                // Example: Assuming the received data is a UTF-8 encoded string
                let received_str = String::from_utf8_lossy(&buffer);
                println!("Received: {}", received_str);

                // Reply to the sender
                let response = "Hello from the server!";
                writer.write_all(response.as_bytes()).await.unwrap();
                println!("Replied with: {}", response);

                // Set up the next receive operation
                receive_operation = Box::pin(receive_wrapper(next_reader));
            }
            _command = receiver.recv() => {
                // Parse the command and send it with writer.write()
            }
        }
    }
}

async fn select_arc_example() {
    let value = Arc::new(Mutex::new(0));
    loop {
        tokio::select! {
            _ = async {
                // Branch 1
                {
                    let mut temp_value = value.lock().await;
                    *temp_value += 1;
                    println!("Branch 1: {}", *temp_value);
                }
                sleep(Duration::from_secs(1)).await;
            } => {
                println!("branch 1");
                let temp_value = value.lock().await;
                println!("branch 1, temp_value: {}", *temp_value);
                if *temp_value >= 10 {
                    break
                }
            },
            _ = async {
                // Branch 2
                {
                    let mut temp_value = value.lock().await;
                    *temp_value += 2;
                    println!("Branch 2: {}", *temp_value);
                }
                sleep(Duration::from_secs(1)).await;
            } => {
                println!("branch 2");
                let temp_value = value.lock().await;
                println!("branch 2, temp_value: {}", *temp_value);
                if *temp_value >= 10 {
                    break
                }
            },
        }
    }
}

// copy from https://github.com/tokio-rs/tokio/discussions/3740
// modify by chatgpt
