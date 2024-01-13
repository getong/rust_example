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

  // fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
  //   unsafe { self.map_unchecked_mut(|s| &mut s.inner) }
  //     .poll(cx)
  //     .map(|_| ())
  // }

  fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
    let box_struct = Pin::into_inner(self);
    Pin::new(&mut box_struct.inner).poll(cx)
  }
}

#[tokio::main]
async fn main() {
  let box_struct = BoxStruct::new();
  box_struct.await;
}
