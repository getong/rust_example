// use tokio::task_local;

tokio::task_local! {
    static NUMBER: u32;
}

#[tokio::main]
async fn main() {
  NUMBER
    .scope(1, async move {
      assert_eq!(NUMBER.get(), 1);
    })
    .await;

  NUMBER
    .scope(2, async move {
      assert_eq!(NUMBER.get(), 2);

      NUMBER
        .scope(3, async move {
          assert_eq!(NUMBER.get(), 3);
        })
        .await;
    })
    .await;
}
