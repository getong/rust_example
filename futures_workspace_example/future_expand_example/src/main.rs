use std::future::Future;
use std::io::Write;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::{fmt, fs};

enum WriteHelloFile {
  Init(String),
  AwaitingCreate(Pin<Box<dyn Future<Output = Result<fs::File, std::io::Error>>>>),
  AwaitingWrite(Pin<Box<dyn Future<Output = Result<(), std::io::Error>>>>),
  Done,
}

impl fmt::Debug for WriteHelloFile {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      WriteHelloFile::Init(s) => write!(f, "Init({})", s),
      WriteHelloFile::AwaitingCreate(_) => write!(f, "AwaitingCreate(...)"),
      WriteHelloFile::AwaitingWrite(_) => write!(f, "AwaitingWrite(...)"),
      WriteHelloFile::Done => write!(f, "Done"),
    }
  }
}

impl PartialEq for WriteHelloFile {
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (WriteHelloFile::Init(s1), WriteHelloFile::Init(s2)) => s1 == s2,
      (WriteHelloFile::AwaitingCreate(_), WriteHelloFile::AwaitingCreate(_)) => true,
      (WriteHelloFile::AwaitingWrite(_), WriteHelloFile::AwaitingWrite(_)) => true,
      (WriteHelloFile::Done, WriteHelloFile::Done) => true,
      _ => false,
    }
  }
}

impl WriteHelloFile {
  pub fn new(name: impl Into<String>) -> Self {
    Self::Init(name.into())
  }
}

impl Future for WriteHelloFile {
  type Output = Result<(), std::io::Error>;
  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    let this = self.get_mut();
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
      WriteHelloFile::Done => return Poll::Ready(Ok(())),
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
  loop {
    match (&mut write_file).await {
      Ok(_) => {
        if write_file == WriteHelloFile::Done {
          println!("done");
          break;
        }
      }
      Err(e) => {
        println!("error reason, e: {:?}", e);
        break;
      }
    }
  }
}
