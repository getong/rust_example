use tokio::sync::OnceCell;

async fn some_computation() -> u32 {
    1 + 1
}

static ONCE: OnceCell<u32> = OnceCell::const_new();

async fn get_global_integer() -> &'static u32 {
    ONCE.get_or_init(|| async { 1 + 1 }).await
}

#[tokio::main]
async fn main() {
    let result = ONCE.get_or_init(some_computation).await;
    assert_eq!(*result, 2);

    let result = get_global_integer().await;
    assert_eq!(*result, 2);
}
