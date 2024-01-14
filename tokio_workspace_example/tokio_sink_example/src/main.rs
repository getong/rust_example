use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::net::{
  tcp::{OwnedReadHalf, OwnedWriteHalf},
  TcpListener, TcpStream,
};
use tokio_serde::{formats::Json, Framed};
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};

#[derive(Serialize, Deserialize, Debug)]
struct MyMessage {
  field: String,
}

type WrappedStream = FramedRead<OwnedReadHalf, LengthDelimitedCodec>;
type WrappedSink = FramedWrite<OwnedWriteHalf, LengthDelimitedCodec>;

// We use the unit type in place of the message types since we're
// only dealing with one half of the IO
type SerStream = Framed<WrappedStream, MyMessage, (), Json<MyMessage, ()>>;
type DeSink = Framed<WrappedSink, (), MyMessage, Json<(), MyMessage>>;

fn wrap_stream(stream: TcpStream) -> (SerStream, DeSink) {
  let (read, write) = stream.into_split();
  let stream = WrappedStream::new(read, LengthDelimitedCodec::new());
  let sink = WrappedSink::new(write, LengthDelimitedCodec::new());
  (
    SerStream::new(stream, Json::default()),
    DeSink::new(sink, Json::default()),
  )
}

#[tokio::main]
async fn main() {
  let listener = TcpListener::bind("0.0.0.0:8080")
    .await
    .expect("Failed to bind server to addr");

  tokio::task::spawn(async move {
    let (stream, _) = listener
      .accept()
      .await
      .expect("Failed to accept incoming connection");

    let (mut stream, mut sink) = wrap_stream(stream);

    println!(
      "Server received: {:?}",
      stream
        .next()
        .await
        .expect("No data in stream")
        .expect("Failed to parse ping")
    );

    sink
      .send(MyMessage {
        field: "pong".to_owned(),
      })
      .await
      .expect("Failed to send pong");
  });

  let stream = TcpStream::connect("127.0.0.1:8080")
    .await
    .expect("Failed to connect to server");

  let (mut stream, mut sink) = wrap_stream(stream);

  sink
    .send(MyMessage {
      field: "ping".to_owned(),
    })
    .await
    .expect("Failed to send ping to server");

  println!(
    "Client received: {:?}",
    stream
      .next()
      .await
      .expect("No data in stream")
      .expect("Failed to parse pong")
  );
}

// copy from https://stackoverflow.com/questions/72247044/how-to-turn-a-tokio-tcpstream-into-a-sink-stream-of-serializable-deserializable
