use tokio::sync::OnceCell;

async fn some_computation() -> u32 {
    1 + 1
}

static ONCE: OnceCell<u32> = OnceCell::const_new();

#[tokio::main]
async fn main() {
    let result = ONCE.get_or_init(some_computation).await;
    assert_eq!(*result, 2);
}
