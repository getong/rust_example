use std::{
  pin::Pin,
  task::{Context, Poll},
};

use bytes::{Bytes, BytesMut};
use futures::task::noop_waker_ref;
use http_body::{Body, Frame, SizeHint};
use tokio::{
  io::{self, AsyncReadExt, Error},
  net::{TcpListener, TcpStream},
};

#[derive(Debug)]
struct MyBody {
  data: bytes::Bytes,
}

impl MyBody {
  fn new(data: &[u8]) -> MyBody {
    MyBody {
      data: data.to_vec().into(),
    }
  }
}

impl Body for MyBody {
  type Data = bytes::Bytes;
  type Error = std::io::Error;

  fn poll_frame(
    mut self: Pin<&mut Self>,
    _cx: &mut core::task::Context<'_>,
  ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
    if !self.data.is_empty() {
      let len = self.data.len().min(1024);
      let data = self.data.split_to(len);
      Poll::Ready(Some(Ok(Frame::data(data))))
    } else {
      Poll::Ready(None)
    }
  }

  fn is_end_stream(&self) -> bool {
    self.data.is_empty()
  }

  fn size_hint(&self) -> SizeHint {
    let exact_size = self.data.len() as u64;
    let mut hint = SizeHint::new();
    hint.set_lower(exact_size);
    hint.set_upper(exact_size); // Optional if you want to specify an exact size
    hint
  }
}

async fn handle_connection(mut stream: TcpStream) -> Result<(), io::Error> {
  // Create a buffer for reading data from the stream.
  let mut buffer = [0; 1024]; // Adjust size as needed.

  // Read data from the stream into the buffer.
  // This reads up to buffer's length of bytes.
  let n = stream.read(&mut buffer).await?;

  // Only take the portion of the buffer that was filled with read data.
  let body_bytes = &buffer[.. n];

  // Create a new instance of MyBody with the read bytes.
  // Since MyBody::new expects a &[u8], and body_bytes is &[u8], this matches.
  let my_body = MyBody::new(body_bytes);

  // Process the body. Note: You'll need to adjust the process_body function to match expected
  // types. Assuming process_body now returns a Result<Bytes, io::Error> for simplicity.
  let processed_result = process_body(Box::pin(my_body)).await?;

  // Print the processed result. Assuming it's of a type that implements Debug.
  println!("processed_result: {:?}", processed_result);

  Ok(())
}

async fn async_server() -> Result<(), Box<dyn std::error::Error>> {
  let listener = TcpListener::bind("127.0.0.1:8080").await?;

  loop {
    let (stream, _) = listener.accept().await?;
    tokio::spawn(async move {
      if let Err(e) = handle_connection(stream).await {
        eprintln!("Failed to handle connection: {}", e);
      }
    });
  }
}

async fn process_body(
  mut body: Pin<Box<dyn Body<Data = Bytes, Error = Error> + Send + Unpin>>,
) -> Result<Bytes, Error> {
  let mut context = Context::from_waker(noop_waker_ref());
  let mut all_data = BytesMut::new();

  loop {
    match body.as_mut().poll_frame(&mut context) {
      Poll::Ready(Some(Ok(frame))) => {
        if let Some(data) = frame.data_ref() {
          all_data.extend_from_slice(&data);
        }
      }
      Poll::Ready(Some(Err(e))) => return Err(e),
      Poll::Ready(None) => break, // End of the stream.
      Poll::Pending => continue,  /* This line simplifies handling, but you need proper async
                                    * handling. */
    }
  }

  Ok(all_data.freeze()) // Convert accumulated data into Bytes
}

// curl http://localhost:8080
#[tokio::main]
async fn main() {
  println!("listen to http://localhost:8080");
  if let Err(e) = async_server().await {
    eprintln!("Server error: {}", e);
  }
}
