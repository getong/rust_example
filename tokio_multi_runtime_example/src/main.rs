use std::thread;
use std::time::Duration;
use tokio::runtime::Runtime;

fn main() {
    // 在第一个线程内创建一个多线程的runtime
    let t1 = thread::spawn(|| {
        let _rt = Runtime::new().unwrap();
        thread::sleep(Duration::from_secs(10));
    });

    // 在第二个线程内创建一个多线程的runtime
    let t2 = thread::spawn(|| {
        let _rt = Runtime::new().unwrap();
        thread::sleep(Duration::from_secs(10));
    });

    t1.join().unwrap();
    t2.join().unwrap();
}
