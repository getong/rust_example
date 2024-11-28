trait IteratorExt {
  fn for_each_async<'a, Ctxt, T, F>(
    self,
    mut captured: Ctxt,
    mut f: F,
  ) -> impl 'a + Send + std::future::Future<Output = ()>
  where
    T: 'a + Send,
    Ctxt: 'a + Send,
    Self: 'a + Send + Iterator<Item = T> + Sized,
    F: 'a
      + Send
      + FnMut(&mut Ctxt, T) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + '_>>,
  {
    async move {
      for value in self {
        f(&mut captured, value).await;
      }
    }
  }
  fn map_collect_async<'a, Ctxt, T, U, F, Ret>(
    self,
    mut captured: Ctxt,
    mut f: F,
  ) -> impl 'a + Send + std::future::Future<Output = Ret>
  where
    T: 'a + Send,
    U: 'a + Send,
    Ctxt: 'a + Send,
    Self: 'a + Send + Iterator<Item = U> + Sized,
    Ret: 'a + Send + FromIterator<T>,
    F: 'a
      + Send
      + FnMut(&mut Ctxt, U) -> std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + '_>>,
  {
    async move {
      let mut ret = match self.size_hint().1 {
        Some(cap) => Vec::with_capacity(cap),
        None => Vec::new(),
      };
      for value in self {
        ret.push(f(&mut captured, value).await);
      }
      ret.into_iter().collect()
    }
  }
}
impl<I: Iterator> IteratorExt for I {}

#[tokio::main]
async fn main() {
  let numbers = vec![1, 2, 3];

  let mut foo = Foo { count: 0 };

  numbers
    .iter()
    .for_each_async(&mut foo, move |foo_ref, &i| {
      Box::pin(async move {
        foo_ref.add(i).await; //
      })
    })
    .await;

  println!("{numbers:?}");
  println!("{foo:?}");

  let ret: Vec<_> = numbers
    .iter()
    .map_collect_async(&mut foo, move |foo_ref, &i| {
      Box::pin(async move {
        foo_ref.add(i).await;
        i
      })
    })
    .await;

  println!("{ret:?}");
  println!("{foo:?}");
}

#[derive(Debug)]
struct Foo {
  count: i32,
}
impl Foo {
  async fn add(&mut self, i: i32) {
    self.count += i;
  }
}

// F: 'a + Send
//   + for<'r> FnMut(&'r mut Ctxt, T) -> std::pin::Pin< Box<dyn std::future::Future<Output = ()> +
//     Send + 'r>, >
//   这个的精髓在于这个类型。
//   一是Future的生命周期'r需要跟着传入的&'r mut Ctxt走，而这个'r不能加在FnMut本身的&mut
// self上，所以这个Ctxt省不了。   二是为了加上这个'r，由于在where
// clause中不支持FnMut的返回值中使用impl Future，所以只能用Pin<Box<dyn Future>>。
//   三是由于用了Pin<Box>，所以不能再用nightly的async closure这个特性，因为async
// closure不支持自定义返回的Future的类型。   综上，因为Rust目前缺两个功能，
// 所以实现这个的方式稍微有点绕。

// copy from https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=cf9c73044c7c28bd5b5042a92f6997b3
