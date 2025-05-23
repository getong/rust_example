use tokio::{
  fs::File,
  io::{self, AsyncReadExt},
};

#[tokio::main]
async fn main() -> io::Result<()> {
  let mut f = File::open("src/main.rs").await?;
  let mut buffer = Vec::new();

  // read the whole file
  f.read_to_end(&mut buffer).await?;
  println!("buffer : {:?}", &buffer);
  println!("buffer : {}", String::from_utf8_lossy(&buffer));
  Ok(())
}
