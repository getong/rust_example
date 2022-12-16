use async_trait::async_trait;
use std::sync::Arc;

#[async_trait]
trait AsyncTrait {
    async fn get_number(&self) -> i32;
}

#[async_trait]
impl AsyncTrait for i32 {
    async fn get_number(&self) -> i32 {
        *self
    }
}

async fn print_the_number(from: Arc<dyn AsyncTrait>) {
    let number = from.get_number().await;
    println!("The number is {number}");
}

#[tokio::main]
async fn main() {
    let number_getter = Arc::new(42);
    print_the_number(number_getter).await;
}
