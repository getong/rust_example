use std::{
  future::Future,
  pin::Pin,
  task::{Context, Poll},
};

struct BoxStruct {
  inner: Pin<Box<dyn Future<Output = ()>>>,
}

impl BoxStruct {
  pub fn new() -> Self {
    let f = Box::pin(async { println!("hello world") });
    Self { inner: f }
  }
}

impl Future for BoxStruct {
  type Output = ();

  fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
    let box_struct = &mut *self;

    let result = box_struct.inner.as_mut().poll(cx);
    box_struct.inner = Box::pin(async { println!("See you") });
    return result;
  }
}

#[tokio::main]
async fn main() {
  let mut box_struct = BoxStruct::new();
  (&mut box_struct).await;
  (&mut box_struct).await;
}
