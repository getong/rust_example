use std::time::Duration;
use tryhard::RetryFutureConfig;

// some async function that can fail
async fn read_file(path: &str) -> Result<String, std::io::Error> {
    // ...
    println!("path:{:?}", path);
    Ok("hello".to_string())
}

#[tokio::main]
async fn main() {
    // println!("Hello, world!");

    let contents = tryhard::retry_fn(|| read_file("Cargo.toml"))
        .retries(10)
        .exponential_backoff(Duration::from_millis(10))
        .max_delay(Duration::from_secs(1))
        .await
        .unwrap();

    println!("contents:{:?}", contents);
    // assert!(contents.contains("tryhard"));

    let config = RetryFutureConfig::new(10)
        .exponential_backoff(Duration::from_millis(10))
        .max_delay(Duration::from_secs(3));

    tryhard::retry_fn(|| read_file("Cargo.toml"))
        .with_config(config)
        .await
        .unwrap();

    // retry another future in the same way
    _ = tryhard::retry_fn(|| read_file("src/main.rs"))
        .with_config(config)
        .await
        .unwrap();
}
