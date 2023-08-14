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

async fn print_the_number_static_dispatch(from: impl AsyncTrait) {
    let number = from.get_number().await;
    println!("The number is {number}");
}

#[tokio::main]
async fn main() {
    let number_getter = Arc::new(42);
    print_the_number(number_getter).await;

    print_the_number_static_dispatch(3_i32).await;
}

// copy from https://theincredibleholk.org/blog/2022/04/18/how-async-functions-in-traits-could-work-in-rustc/
