use futures::future::BoxFuture;
//use futures::future::Future;
use futures::FutureExt;

fn foo() -> BoxFuture<'static, u32> {
  async move { 42 }.boxed()
}

async fn bar() {
  let _u: u32 = foo().await;
  println!("_u is {:?}", _u);
}

#[tokio::main]
async fn main() {
  bar().await;

  let other_u: u32 = foo().await;
  println!("other_u is {:?}", other_u);
}

// copy from https://users.rust-lang.org/t/how-to-unbox-deference-a-boxfuture/56691
