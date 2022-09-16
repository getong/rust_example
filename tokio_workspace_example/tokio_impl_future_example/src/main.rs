use futures::Future;
use tokio::time::{sleep, Duration};

fn call_api_two() -> impl Future<Output = String> {
    async {
        sleep(Duration::from_secs(1)).await;
        "Two".to_string()
    }
}

fn get_async_name() -> impl Future<Output = String> {
    let name: String = "John".to_string();
    async move { format!("Hello {} Doe", name) }
}

#[tokio::main]
async fn main() {
    // println!("Hello, world!");
    let two = call_api_two().await;
    println!("two: {}", two);

    let name = get_async_name().await;
    println!("name: {}", name);
}
