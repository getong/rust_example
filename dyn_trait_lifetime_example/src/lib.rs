use std::future::Future;

pub trait T: Send + Sync {
  fn call(&self) -> impl Future<Output = i32> + Send;
}

impl<A> T for Box<A>
where
  A: T + ?Sized,
{
  async fn call(&self) -> i32 {
    self.as_ref().call().await
  }
}

trait A: Send + Sync {}

// dyn A here means dyn A + 'static , it must add + '_ here
impl T for dyn A + '_ {
  async fn call(&self) -> i32 {
    10
  }
}

pub struct B(Box<dyn A>);

impl T for B {
  async fn call(&self) -> i32 {
    // 把 Box<dyn A> 转换成 &dyn A 可以编译通过，但我不要这种
    // self.0.as_ref().call().await
    self.0.call().await
  }
}

// copy from https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=f1544d058e41189bd792ec78b1fb7f5a
// also see https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=39bacb7d01c65b87f1a3e70c1be78645
// also see https://doc.rust-lang.org/reference/lifetime-elision.html#default-trait-object-lifetimes
