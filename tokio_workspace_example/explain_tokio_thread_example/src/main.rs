use tokio;

fn main() {
    let _rt = tokio::runtime::Runtime::new().unwrap();
    std::thread::sleep(std::time::Duration::from_secs(10));
}
