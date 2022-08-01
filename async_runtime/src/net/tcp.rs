use std::{
    future::Future,
    // io::{self, Read, Write},
    io::{self, Read},
    net::{Shutdown, TcpStream},
    pin::Pin,
    task::{Context, Poll},
};
/// just to wrap a TcpStream in order to implement  different interfaces
/// User can use this type like below
/// ```
/// async {
///     let mut stream = AsyncTcpStream::connect();
///     let mut buf = vec![0:1000];
///     let num_bytes = stream.read(&buf).await;
///     stream.close();
/// }
/// ```
pub struct AsyncTcpStream {
    stream: TcpStream,
}

impl AsyncTcpStream {
    pub fn connect(addr: &str) -> Self {
        let stream = TcpStream::connect(addr).unwrap();
        // set to nonblocking so that we can control based on return status
        stream.set_nonblocking(true).unwrap();
        Self { stream }
    }
    pub fn close(&self) {
        // shutdown connection properly
        self.stream.shutdown(Shutdown::Both).unwrap();
    }
    /// return a future for polling
    pub fn read<'a, 'b>(&'a mut self, buf: &'b mut [u8]) -> ReadFuture<'a, 'b> {
        ReadFuture {
            stream: &self.stream,
            buf,
        }
    }
}

pub struct ReadFuture<'a, 'b> {
    stream: &'a TcpStream,
    buf: &'b mut [u8],
}

impl<'a, 'b> Future for ReadFuture<'a, 'b> {
    type Output = usize;
    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        let f = self.get_mut();
        match f.stream.read(&mut f.buf) {
            Ok(n_bytes) => Poll::Ready(n_bytes),
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => Poll::Pending,
            Err(e) => panic!("Future read error! {e:?}"),
        }
    }
}
