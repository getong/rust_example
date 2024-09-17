use std::error::Error;
use std::io;
use std::future::Future;
use std::pin::Pin;
use stubborn_io::tokio::{StubbornIo, UnderlyingIo};
use tokio::net::TcpStream;
use tokio::io::{AsyncWriteExt, AsyncWrite};
use derive_deref::{Deref, DerefMut};
use tokio::time::{Duration, sleep};

#[derive(Deref, DerefMut)]
struct DurableTCPStream(TcpStream);

impl AsyncWrite for DurableTCPStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, io::Error>> {
        <TcpStream as AsyncWrite>::poll_write(Pin::new(&mut self.0), cx, buf)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), io::Error>> {
        <TcpStream as AsyncWrite>::poll_flush(Pin::new(&mut self.0), cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), io::Error>> {
        <TcpStream as AsyncWrite>::poll_shutdown(Pin::new(&mut self.0), cx)
    }

    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> std::task::Poll<Result<usize, io::Error>> {
        <TcpStream as AsyncWrite>::poll_write_vectored(Pin::new(&mut self.0), cx, bufs)
    }

    fn is_write_vectored(&self) -> bool {
        <TcpStream as AsyncWrite>::is_write_vectored(&self.0)
    }
}

impl UnderlyingIo<String> for DurableTCPStream {
    fn establish(addr: String) -> Pin<Box<dyn Future<Output = io::Result<Self>> + Send>> {
        Box::pin(async move {
            let parts: Vec<&str> = addr.split('/').collect();
            println!("connecting to {}", parts[0]);
            let mut stream = TcpStream::connect(parts[0]).await?;

            if parts.len() > 1 {
                // hello message was specified, use it
                println!("sending login: {}", parts[1]);
                stream.write_all(format!("{}\n", parts[1]).as_ref()).await?;
            }
            Ok(DurableTCPStream(stream))
        })
    }
}

type StubbornTCP = StubbornIo<DurableTCPStream, String>;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut stream = StubbornTCP::connect(String::from("localhost:9999/logincmd")).await?;

    loop {
        println!("sending msg");
        match stream.write_all(b"hello world!\n").await {
            Ok(_) => (),
            Err(e) => {
                println!("{}", e);
            }
        }
        sleep(Duration::from_millis(1000)).await;
    }
}

// copy from https://github.com/craftytrickster/stubborn-io/issues/33