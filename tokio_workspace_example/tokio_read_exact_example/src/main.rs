use tokio::fs::File;
use tokio::io::{self, AsyncReadExt};

#[tokio::main]
async fn main() -> io::Result<()> {
  let mut f = File::open("/dev/urandom").await?;
  let mut buffer = [0; 10];

  // read exactly 10 bytes
  f.read_exact(&mut buffer).await?;
  Ok(())
}
