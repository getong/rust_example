use std::fs;
use std::future::Future;
use std::io::{Error, Write};
use std::pin::Pin;
use std::task::{Context, Poll};

enum WriteHelloFile {
  Init(String),
  AwaitingCreate(Pin<Box<dyn Future<Output = Result<fs::File, std::io::Error>>>>),
  AwaitingWrite(Pin<Box<dyn Future<Output = Result<(), std::io::Error>>>>),
  Done,
}

impl WriteHelloFile {
  pub fn new(name: impl Into<String>) -> Self {
    Self::Init(name.into())
  }
}

impl Future for WriteHelloFile {
  type Output = Result<(), std::io::Error>;
  fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    let this = self.as_mut().get_mut();
    match this {
      WriteHelloFile::Init(name) => {
        let name_clone = name.clone();
        let fut = Box::pin(async { fs::File::create(name_clone) });
        *this = WriteHelloFile::AwaitingCreate(fut);
        return Poll::Ready(Ok(()));
      }
      WriteHelloFile::AwaitingCreate(fut) => match fut.as_mut().poll(cx) {
        Poll::Ready(Ok(mut v)) => {
          let fut = Box::pin(async move { v.write_all(b"hello world!") });
          *this = WriteHelloFile::AwaitingWrite(fut);
          return Poll::Ready(Ok(()));
        }
        Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
        Poll::Pending => return Poll::Pending,
      },
      WriteHelloFile::AwaitingWrite(fut) => match fut.as_mut().poll(cx) {
        Poll::Ready(Ok(_)) => {
          *this = WriteHelloFile::Done;
          return Poll::Ready(Ok(()));
        }
        Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
        Poll::Pending => return Poll::Pending,
      },
      WriteHelloFile::Done => return Poll::Ready(Err(Error::from(std::io::ErrorKind::Other))),
    }
  }
}

#[tokio::main]
async fn main() {
  let mut write_file = WriteHelloFile::new("abc.txt");
  // _ = (&mut write_file).await;
  // _ = (&mut write_file).await;
  // _ = (&mut write_file).await;
  // _ = (&mut write_file).await;
  while let Ok(_) = (&mut write_file).await {}
  println!("done");
}
