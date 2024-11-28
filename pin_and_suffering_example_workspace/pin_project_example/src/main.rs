use std::{
  future::Future,
  pin::Pin,
  task::{Context, Poll},
};

use pin_project::pin_project;
use tokio::{
  fs::File,
  io::{AsyncRead, AsyncReadExt, ReadBuf},
  time::{Duration, Instant, Sleep},
};

// impl<R> SlowRead<R>
// where
//    R: Unpin,
//{
//    // 👇 now takes pinned mutable reference to Self, and returns an option
//    fn take_inner(self: Pin<&mut Self>) -> Option<R> {
//        self.reader.take()
//    }
//}

#[pin_project]
struct SlowRead<R> {
  #[pin]
  reader: R,
  #[pin]
  sleep: Sleep,
}

impl<R> SlowRead<R> {
  fn new(reader: R) -> Self {
    Self {
      reader,
      sleep: tokio::time::sleep(Default::default()),
    }
  }
}

impl<R> AsyncRead for SlowRead<R>
where
  R: AsyncRead + Unpin,
{
  fn poll_read(
    self: Pin<&mut Self>,
    cx: &mut Context<'_>,
    buf: &mut ReadBuf<'_>,
  ) -> Poll<std::io::Result<()>> {
    //       👇            👇
    let mut this = self.project();

    match this.sleep.as_mut().poll(cx) {
      Poll::Ready(_) => {
        this.sleep.reset(Instant::now() + Duration::from_millis(25));
        this.reader.poll_read(cx, buf)
      }
      Poll::Pending => Poll::Pending,
    }
  }
}

#[tokio::main]
async fn main() -> Result<(), tokio::io::Error> {
  let mut buf = vec![0u8; 128 * 1024];
  let f = File::open("/dev/urandom").await?;

  let f = SlowRead::new(f);
  pin_utils::pin_mut!(f);

  let before = Instant::now();
  f.read_exact(&mut buf).await?;
  println!("Read {} bytes in {:?}", buf.len(), before.elapsed());

  // f.take_inner().unwrap();

  let before = Instant::now();
  f.read_exact(&mut buf).await?;
  println!("Read {} bytes in {:?}", buf.len(), before.elapsed());

  Ok(())
}
