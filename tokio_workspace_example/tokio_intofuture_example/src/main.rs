use std::future::IntoFuture;

#[tokio::main]
async fn main() {
    let v = async { "meow" };
    let fut = v.into_future();
    assert_eq!("meow", fut.await);
}
