use async_std::task;
use std::time::Duration;

struct Guard;
impl Drop for Guard {
    fn drop(&mut self) {
        println!("2");
    }
}

async fn foo(_guard: Guard) {
    println!("3");
    task::sleep(Duration::from_secs(1)).await;
    println!("4");
}

fn main() {
    println!("1");
    let guard = Guard {};
    let fut = foo(guard);
    drop(fut);
    println!("done");
}
