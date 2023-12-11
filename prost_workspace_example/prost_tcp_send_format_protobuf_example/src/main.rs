use bytes;
use bytes::BufMut;
use bytes::BytesMut;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
mod protos;
use prost::{Message, Name};
use protos::counter_number;

// telnet localhost 12345

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let listener = TcpListener::bind("127.0.0.1:12345").await?;

  loop {
    let (mut socket, _) = listener.accept().await?;

    tokio::spawn(async move {
      let mut buf = [0; 1024];

      send_random_msg(&mut socket).await;

      // In a loop, read data from the socket and write the data back.
      loop {
        let n = match socket.read(&mut buf).await {
          // socket closed
          Ok(n) if n == 0 => return,
          Ok(n) => n,
          Err(e) => {
            eprintln!("failed to read from socket; err = {:?}", e);
            return;
          }
        };

        // Write the data back
        if let Err(e) = socket.write_all(&buf[0..n]).await {
          eprintln!("failed to write to socket; err = {:?}", e);
          return;
        }
      }
    });
  }
}

pub fn encode_header_len(message_id_len: u32, body_len: u32) -> u32 {
  return (0 << 31) | (message_id_len << 20) | body_len;
}

pub fn body_len(size: u32) -> u32 {
  return size & 0xFFFFF;
}

pub fn message_id_len(size: u32) -> u32 {
  return (size >> 20) & 0x3f;
}

async fn send_random_msg(socket: &mut TcpStream) {
  let sample_schema = counter_number::SampleSchema {
    sample_field_one: true,
    sample_field_two: true,
  };
  let msg = counter_number::ReadRequest {
    letter: "hello world".to_owned(),
    before_number: 1_i32,
    dummy_one: 2_u32,
    dummy_two: Some(sample_schema),
    dummy_three: vec![1, 2, 3],
  };

  let bytes = msg.encode_to_vec();
  let body_len = bytes.len();
  let message_id = counter_number::ReadRequest::full_name().to_owned();
  let message_id_len = message_id.len();
  let header = encode_header_len(message_id_len as u32, body_len as u32);

  let mut total_buf = BytesMut::new();
  total_buf.reserve(4 + message_id_len + body_len);
  total_buf.put_u32(header);
  total_buf.put_slice(message_id.as_bytes());
  total_buf.put_slice(&bytes);

  if let Err(e) = socket.write_all(&total_buf).await {
    eprintln!("failed to write to socket; err = {:?}", e);
  }
}
