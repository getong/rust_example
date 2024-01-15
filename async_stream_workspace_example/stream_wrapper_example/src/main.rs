use anyhow::Result;
use pin_project::pin_project;
use std::{
  pin::Pin,
  task::{Context, Poll},
};
use tokio::{
  fs::File,
  io::{AsyncRead, AsyncReadExt, ReadBuf},
};

#[pin_project]
struct FileWrapper {
  #[pin]
  file: File,
}

impl FileWrapper {
  pub async fn try_new(name: &str) -> Result<Self> {
    let file = File::open(name).await?;
    Ok(Self { file })
  }
}

impl AsyncRead for FileWrapper {
  fn poll_read(
    self: Pin<&mut Self>,
    cx: &mut Context<'_>,
    buf: &mut ReadBuf<'_>,
  ) -> Poll<std::io::Result<()>> {
    self.project().file.poll_read(cx, buf)
  }
}

#[tokio::main]
async fn main() -> Result<()> {
  let mut file = FileWrapper::try_new("./Cargo.toml").await?;
  let mut buffer = String::new();
  file.read_to_string(&mut buffer).await?;
  println!("{}", buffer);
  Ok(())
}

// copy from https://github.com/tyrchen/geektime-rust