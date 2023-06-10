use async_lock::Mutex;

#[tokio::main]
async fn main() {
    // println!("Hello, world!");

    let m = Mutex::new(1);

    let mut guard = m.lock().await;
    *guard = 2;

    assert!(m.try_lock().is_none());
    drop(guard);
    assert_eq!(*m.try_lock().unwrap(), 2);
}
