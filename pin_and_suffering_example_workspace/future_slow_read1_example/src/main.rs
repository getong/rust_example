use std::{
    pin::Pin,
    task::{Context, Poll},
};

use tokio::{
    fs::File,
    io::{AsyncRead, AsyncReadExt, ReadBuf},
    time::Instant,
};

struct SlowRead<R> {
    reader: Pin<Box<R>>,
}

impl<R> SlowRead<R> {
    // fn new(reader: R) -> Self {
    //    Self {
    //        reader: Box::pin(reader),
    //    }
    // }
}

impl<R> AsyncRead for SlowRead<R>
where
    R: AsyncRead,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        self.reader.as_mut().poll_read(cx, buf)
    }
}

#[tokio::main]
async fn main() -> Result<(), tokio::io::Error> {
    let mut buf = vec![0u8; 128 * 1024];
    let mut f = File::open("/dev/urandom").await?;
    let before = Instant::now();
    f.read_exact(&mut buf).await?;
    println!("Read {} bytes in {:?}", buf.len(), before.elapsed());

    Ok(())
}
