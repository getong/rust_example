use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;

#[tokio::main]
async fn main() {
    let _ = io_split();
    let _ = net_split();
}

async fn io_split() -> Result<(), Box<dyn std::error::Error>> {
    let stream = TcpStream::connect("localhost:8080").await?;
    let (mut read, _write) = tokio::io::split(stream);

    tokio::spawn(async move {
        loop {
            let mut buf = [0u8; 32];
            read.read(&mut buf).await.unwrap();
            println!("{:?}", std::str::from_utf8(&buf));
        }
    });

    Ok(())
}

async fn net_split() -> Result<(), Box<dyn std::error::Error>> {
    // note here, the stream is not decleared as above, but inside the spawn() method
    // let mut stream = TcpStream::connect("localhost:8080").await.unwrap();
    // let (mut read, _write) = stream.split();
    tokio::spawn(async move {
        let mut stream = TcpStream::connect("localhost:8080").await.unwrap();
        let (mut read, _write) = stream.split();
        loop {
            let mut buf = [0u8; 32];
            read.read(&mut buf).await.unwrap();
            println!("{:?}", std::str::from_utf8(&buf));
        }
    });

    Ok(())
}
