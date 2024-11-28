use std::{
  future::Future,
  pin::Pin,
  task::{Context, Poll},
};

use tokio::{fs, io::AsyncWriteExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  let filename = "/tmp/async_internals";
  write_hello_file_async(filename).await?;

  Ok(())
}

async fn write_hello_file_async(name: &str) -> anyhow::Result<()> {
  let mut file = fs::File::create(name).await?;
  file.write_all(b"hello world!").await?;

  Ok(())
}

enum WriteFileAsync {
  WaitForCreate(CreateFut),
  WaitForWrite(WriteFut),
}

struct CreateFut(Pin<Box<dyn Future<Output = Result<fs::File, std::io::Error>>>>);

struct WriteFut {
  f: Pin<Box<dyn Future<Output = Result<(), std::io::Error>>>>,
}

impl Future for WriteFileAsync {
  type Output = Result<(), std::io::Error>;

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    let this = unsafe { self.get_unchecked_mut() };
    match this {
      WriteFileAsync::WaitForCreate(fut) => match Pin::new(&mut fut.0).poll(cx) {
        Poll::Ready(Ok(mut f)) => {
          let fut = WriteFut {
            f: Box::pin(f.write_all(b"hello world!")),
          };

          *this = WriteFileAsync::WaitForWrite(fut);
          return Pin::new(this).poll(cx);
        }
        Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
        Poll::Pending => return Poll::Pending,
      },
      WriteFileAsync::WaitForWrite(fut) => match Pin::new(&mut fut.f).poll(cx) {
        Poll::Ready(Ok(_)) => return Poll::Ready(Ok(())),
        Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
        Poll::Pending => return Poll::Pending,
      },
    }
  }
}
