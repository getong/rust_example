use std::{
  future::Future,
  pin::Pin,
  task::{Context, Poll},
};

use futures_util::{Stream, StreamExt};

#[pin_project::pin_project]
struct BoxedStream<T, F> {
  next: Option<String>,
  inner: Option<Pin<Box<dyn Future<Output = T>>>>,
  generate: F,
}

impl<T, F> Stream for BoxedStream<T, F>
where
  F: Fn(String) -> (Option<String>, Pin<Box<dyn Future<Output = T>>>),
{
  type Item = T;

  fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
    let p = self.as_mut().project();
    if let Some(s) = p.inner {
      return match s.as_mut().poll(cx) {
        Poll::Ready(result) => {
          p.inner.take();
          Poll::Ready(Some(result))
        }
        _ => Poll::Pending,
      };
    }

    if let Some(next) = p.next.take() {
      let (next, future) = (p.generate)(next);
      *p.inner = Some(future);
      *p.next = next;
      return self.poll_next(cx);
    }

    Poll::Ready(None)
  }
}

///////////// Boilerplate to make your code do something ///////////////////

impl<T, F> BoxedStream<T, F>
where
  F: Fn(String) -> (Option<String>, Pin<Box<dyn Future<Output = T>>>),
{
  fn new(generate: F) -> Self {
    let (next, inner) = generate("".to_string());
    Self {
      next,
      inner: Some(inner),
      generate,
    }
  }
}

async fn gen(x: String) -> String {
  format!("{} {}", x.len(), x)
}

#[tokio::main]
async fn main() {
  let mut stream = BoxedStream::new(|mut s: String| {
    let fut = Box::pin(gen(s.clone()));
    if s.len() < 5 {
      s.push('A');
      (Some(s), fut)
    } else {
      (None, fut)
    }
  });

  while let Some(x) = stream.next().await {
    println!("{}", x);
  }
}

// copy from https://stackoverflow.com/questions/77113869/how-can-i-poll-a-pinoptionboxdyn-future