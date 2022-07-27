use std::str;
use std::sync::{Arc, Mutex, MutexGuard};
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};

#[tokio::main]
async fn main() {
    // create balance wrapped in Arc and Mutex for cross thread safety
    let balance = Arc::new(Mutex::new(0.00f32));
    let listener = TcpListener::bind("127.0.0.1:8181").await.unwrap();

    loop {
        let (stream, _) = listener.accept().await.unwrap();
        // Clone the balance Arc and pass it to handler
        let balance = balance.clone();
        tokio::spawn(async move {
            handle_connection(stream, balance).await;
        });
    }
}

async fn handle_connection(mut stream: TcpStream, balance: Arc<Mutex<f32>>) {
    // Read the first 16 characters from the incoming stream.
    let mut buffer = [0; 16];
    stream.read(&mut buffer).await.unwrap();
    // First 4 characters are used to detect HTTP method
    let method_type = match str::from_utf8(&buffer[0..4]) {
        Ok(v) => v,
        Err(e) => panic!("Invalid UTF-8 sequence: {}", e),
    };
    let contents = match method_type {
        "GET " => {
            // before using balance we need to lock it.
            format!("{{\"balance\": {}}}", balance.lock().unwrap())
        }
        "POST" => {
            // Take characters after 'POST /' until whitespace is detected.
            let input: String = buffer[6..16]
                .iter()
                .take_while(|x| **x != 32u8)
                .map(|x| *x as char)
                .collect();
            let balance_update = input.parse::<f32>().unwrap();

            // acquire lock on our balance and update the value
            let mut locked_balance: MutexGuard<f32> = balance.lock().unwrap();
            *locked_balance += balance_update;
            format!("{{\"balance\": {}}}", locked_balance)
        }
        _ => {
            panic!("Invalid HTTP method!")
        }
    };

    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        contents.len(),
        contents
    );
    stream.write(response.as_bytes()).await.unwrap();
    stream.flush().await.unwrap();
}
