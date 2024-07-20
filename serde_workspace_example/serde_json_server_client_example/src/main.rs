use serde::{Deserialize, Serialize};
use std::error::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[derive(Serialize, Deserialize, Debug)]
struct MyStruct {
  field1: String,
  field2: i32,
}

async fn handle_client(mut socket: TcpStream) -> Result<(), Box<dyn Error>> {
  let mut buffer = vec![0; 1024];
  let n = socket.read(&mut buffer).await?;
  let received_json = String::from_utf8(buffer[..n].to_vec())?;
  let received_struct: MyStruct = serde_json::from_str(&received_json)?;

  println!("Server Received: {:?}", received_struct);

  // Optionally, send a response back
  let response_struct = MyStruct {
    field1: "Response".to_string(),
    field2: 42,
  };
  println!("server send {:?}", response_struct);
  let response_json = serde_json::to_string(&response_struct)?;
  socket.write_all(response_json.as_bytes()).await?;

  Ok(())
}

async fn run_server() -> Result<(), Box<dyn Error>> {
  let listener = TcpListener::bind("127.0.0.1:8080").await?;
  println!("Server running on 127.0.0.1:8080");

  loop {
    let (socket, _) = listener.accept().await?;
    tokio::spawn(async move {
      if let Err(e) = handle_client(socket).await {
        eprintln!("Error handling client: {:?}", e);
      }
    });
  }
}

async fn run_client() -> Result<(), Box<dyn Error>> {
  let mut socket = TcpStream::connect("127.0.0.1:8080").await?;
  let my_struct = MyStruct {
    field1: "Hello".to_string(),
    field2: 123,
  };
  println!("client send {:?}", my_struct);
  let json = serde_json::to_string(&my_struct)?;
  socket.write_all(json.as_bytes()).await?;

  let mut buffer = vec![0; 1024];
  let n = socket.read(&mut buffer).await?;
  let received_json = String::from_utf8(buffer[..n].to_vec())?;
  let received_struct: MyStruct = serde_json::from_str(&received_json)?;

  println!("Client Received: {:?}", received_struct);

  Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  tokio::spawn(async {
    if let Err(e) = run_server().await {
      eprintln!("Server error: {:?}", e);
    }
  });

  tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

  if let Err(e) = run_client().await {
    eprintln!("Client error: {:?}", e);
  }

  Ok(())
}
