use tokio::io::{
    self, AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, ReadHalf, WriteHalf,
};
use tokio::net::TcpStream;
use tokio::sync::mpsc::{self, Receiver};

#[tokio::main]
async fn main() {
    // Establish a TCP connection
    let stream = TcpStream::connect("127.0.0.1:8080").await.unwrap();

    // Split the TCP stream into separate read and write halves
    let (read_half, write_half) = io::split(stream);

    // Create an MPSC channel with a capacity of 10
    let (tx, rx) = mpsc::channel::<Vec<u8>>(10);

    // Spawn two tasks to perform reading and writing asynchronously
    spawn_reader_task(read_half);

    spawn_write_task(write_half, rx);

    // Await the completion of both tasks
    // reader_task.await.unwrap();
    // writer_task.await.unwrap();

    let stdin = io::stdin();
    let mut stdin_reader = BufReader::new(stdin);

    println!("echo via tcp, type 'quit' to exit.");

    loop {
        let mut line = String::new();
        match stdin_reader.read_line(&mut line).await {
            Ok(0) => {
                break; // End of input
            }

            Ok(_) => {
                let input = line.trim().to_owned(); // Convert to owned String
                if input == "quit" {
                    break;
                }
                // println!("input:{:?}", input);

                // Send an owned value through the channel
                if let Err(_) = tx.send(input.into_bytes()).await {
                    println!("channel send error");
                }
            }
            Err(err) => {
                eprintln!("Failed to read input: {}", err);
                break;
            }
        }
    }
}

fn spawn_reader_task(read_half: ReadHalf<TcpStream>) {
    let _reader_task = tokio::spawn(async move {
        let mut reader = BufReader::new(read_half);
        let mut buf = vec![0u8; 1024];

        loop {
            match reader.read(&mut buf).await {
                Ok(n) if n > 0 => {
                    // Process the received data
                    println!("Received: {}", String::from_utf8_lossy(&buf[..n]));
                }
                _ => break,
            }
        }
    });
}

fn spawn_write_task(write_half: WriteHalf<TcpStream>, mut rx: Receiver<Vec<u8>>) {
    tokio::spawn(async move {
        let mut writer = tokio::io::BufWriter::new(write_half);
        loop {
            if let Some(data) = rx.recv().await {
                println!("Send: {}", String::from_utf8_lossy(&data));
                _ = writer.write_all(&data).await;
                if let Err(_) = writer.flush().await {
                    println!("send to network error");
                }
            }
        }
    });
}
