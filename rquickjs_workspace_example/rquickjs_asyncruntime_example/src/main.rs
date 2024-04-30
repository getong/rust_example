use rquickjs::{async_with, AsyncContext, AsyncRuntime};

#[tokio::main]
async fn main() {
  let rt = AsyncRuntime::new().unwrap();
  let ctx = AsyncContext::full(&rt).await.unwrap();

  async_with!(&ctx => |ctx|{
    let code_str = "1 + 1";
    let res: i32 = ctx.eval(code_str).unwrap();
    println!("res is {}", res);
    assert_eq!(res,2i32);
  })
  .await;
}
