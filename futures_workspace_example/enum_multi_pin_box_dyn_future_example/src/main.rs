use std::{
  future::Future,
  pin::Pin,
  task::{Context, Poll},
};

enum WriteHelloFile {
  Init(String),
  AwaitingCreate(Pin<Box<dyn Future<Output = ()>>>),
  AwaitingWrite(Pin<Box<dyn Future<Output = ()>>>),
  Done,
}

impl WriteHelloFile {
  pub fn new(name: impl Into<String>) -> Self {
    Self::Init(name.into())
  }
}

impl Future for WriteHelloFile {
  type Output = ();

  fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
    let write_hell_file = Pin::into_inner(self);
    match write_hell_file {
      WriteHelloFile::Init(box_string) => {
        println!("box_string is {}", box_string);
        return Poll::Ready(());
      }
      WriteHelloFile::AwaitingCreate(create_func) => create_func.as_mut().poll(cx),
      WriteHelloFile::AwaitingWrite(write_func) => write_func.as_mut().poll(cx),
      WriteHelloFile::Done => {
        println!("Done!");
        return Poll::Ready(());
      }
    }
  }
}

#[tokio::main]
async fn main() {
  let write_file1 = WriteHelloFile::new("hello world");
  write_file1.await;

  let write_file2 = WriteHelloFile::AwaitingCreate(Box::pin(async { println!("create file") }));
  write_file2.await;

  let write_file3 = WriteHelloFile::AwaitingWrite(Box::pin(async { println!("write file") }));
  write_file3.await;

  let write_file4 = WriteHelloFile::Done;
  write_file4.await;
}
