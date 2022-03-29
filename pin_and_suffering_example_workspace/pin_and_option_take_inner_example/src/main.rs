use std::future::Future;
use std::task::Context;
use tokio::fs::File;
use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt;
use tokio::io::ReadBuf;
use tokio::macros::support::Pin;
use tokio::macros::support::Poll;
use tokio::time::Duration;
use tokio::time::Instant;
use tokio::time::Sleep;

struct SlowRead<R> {
    //       👇 now optional!
    reader: Option<R>,
    sleep: Sleep,
}

impl<R> SlowRead<R> {
    fn new(reader: R) -> Self {
        Self {
            //       👇
            reader: Some(reader),
            sleep: tokio::time::sleep(Default::default()),
        }
    }
}

impl<R> SlowRead<R>
where
    R: Unpin,
{
    // 👇 now takes pinned mutable reference to Self, and returns an option
    fn take_inner(self: Pin<&mut Self>) -> Option<R> {
        // self.reader.take()
        let mut_self = unsafe { self.get_unchecked_mut() };
        mut_self.reader.take()
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
        // pin-project both fields
        let (mut sleep, reader) = unsafe {
            let this = self.get_unchecked_mut();
            (Pin::new_unchecked(&mut this.sleep), &mut this.reader)
        };

        match sleep.as_mut().poll(cx) {
            Poll::Ready(_) => {
                sleep.reset(Instant::now() + Duration::from_millis(25));
                match reader {
                    Some(reader) => {
                        // pin-project option:
                        let reader = unsafe { Pin::new_unchecked(reader) };
                        reader.poll_read(cx, buf)
                    }
                    None => {
                        // simulate EOF
                        Poll::Ready(Ok(()))
                    }
                }
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

    if let Some(mut f) = f.take_inner() {
        let before = Instant::now();
        f.read_exact(&mut buf).await?;
        println!("Read {} bytes in {:?}", buf.len(), before.elapsed());

        return Ok(());
    }

    Err(std::io::Error::new(std::io::ErrorKind::Other, "foo"))
}
