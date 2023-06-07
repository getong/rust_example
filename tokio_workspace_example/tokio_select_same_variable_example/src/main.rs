use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

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

// copy from https://github.com/tokio-rs/tokio/discussions/3740
// modify by chatgpt