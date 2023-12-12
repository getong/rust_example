// use futures::future::poll_fn;
use std::error::Error;
use tokio::io::{self, Interest};
// use tokio::io::{self, ReadBuf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> io::Result<()> {
  let (tx, rx) = oneshot::channel();

  tokio::spawn(async move {
    sleep(Duration::from_secs(100)).await;
    tx.send(()).unwrap();
  });

  let listener = TcpListener::bind("localhost:3465").await?;

  tokio::select! {
      _ = async {
          loop {
              while let Ok((stream, _)) = listener.accept().await {
                  tokio::spawn(async move { process(stream) });
              }
          }

          // Ok::<_, io::Error>(())
      } => {}
      _ = rx => {
          println!("terminating accept loop");
      }
  }

  Ok(())
}

async fn process(stream: TcpStream) -> Result<(), Box<dyn Error>> {
  println!("socket: {:?}", stream);
  // let mut buf = [0; 10];
  // let mut buf = ReadBuf::new(&mut buf);

  // loop {
  //     poll_fn(|cx| stream.poll_peek(cx, &mut buf)).await?;
  //     println!("read {:?}", buf);
  // }
  loop {
    let ready = stream
      .ready(Interest::READABLE | Interest::WRITABLE)
      .await?;

    if ready.is_readable() {
      let mut data = vec![0; 1024];
      // Try to read data, this may still fail with `WouldBlock`
      // if the readiness event is a false positive.
      match stream.try_read(&mut data) {
        Ok(n) => {
          println!("read {} bytes", n);
        }
        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
          continue;
        }
        Err(e) => {
          return Err(e.into());
        }
      }
    }

    if ready.is_writable() {
      // Try to write data, this may still fail with `WouldBlock`
      // if the readiness event is a false positive.
      match stream.try_write(b"hello world") {
        Ok(n) => {
          println!("write {} bytes", n);
        }
        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => continue,
        Err(e) => {
          return Err(e.into());
        }
      }
    }
  }
}
