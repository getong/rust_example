use std::time::Duration;

async fn worker(name: &'static str) {
    let guard = elegant_departure::get_shutdown_guard().await.unwrap();

    println!("[{}] working", name);

    guard.wait().await;
    println!("[{}] shutting down", name);

    tokio::time::sleep(Duration::from_secs(1)).await;
    println!("[{}] done", name);
}

#[tokio::main]
async fn main() {
    tokio::spawn(worker("worker 1"));
    tokio::spawn(worker("worker 2"));

    tokio::signal::ctrl_c().await.unwrap();
    elegant_departure::shutdown().await;
}
