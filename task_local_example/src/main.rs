use task_local::{TaskLocal, WithLocalExt};

#[derive(Default, TaskLocal, Debug)]
struct Context {
  value: i32,
}

#[tokio::main]
async fn main() {
  let b = local();
  println!("b :{:?}", b);

  let c = scope();
  println!("c :{:?}", c);
}

fn local() {
  tokio::spawn(async {
    let a = async {
      // set the local
      Context::local_mut(|ctx| ctx.value = 42);

      // get the local
      let value = Context::local(|ctx| ctx.value);
      println!("{}", value); // prints 42
    }
    .with_local(Context::default())
    .await;
    println!("a:{:?}", a);
  });
}

fn scope() {
  tokio::spawn(async {
    assert!(Context::try_local(|_ctx| {}).is_err());

    async {
      Context::local(|ctx| assert!(ctx.value == 0));
      Context::local_mut(|ctx| ctx.value = 42);
      Context::local(|ctx| assert!(ctx.value == 42));

      async {
        Context::local(|ctx| assert!(ctx.value == 0));
        Context::local_mut(|ctx| ctx.value = 5);
        Context::local(|ctx| assert!(ctx.value == 5));
      }
      .with_local(Context::default())
      .await;

      Context::local(|ctx| assert!(ctx.value == 42));
    }
    .with_local(Context::default())
    .await;

    assert!(Context::try_local(|_ctx| {}).is_err());
  });
}
