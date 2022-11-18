use std::thread::sleep;
use std::time::Duration;
use tokio::runtime;

async fn random_print(num: i32) {
    let r = rand::random::<u64>() % 500u64;
    println!("msg{} will sleep for {}ms", num, r);
    sleep(Duration::from_millis(r));
    println!("msg{} complete", num);
}

fn main() {
    not_a_main_fn();
}

fn not_a_main_fn() {
    let rt = runtime::Builder::new_multi_thread()
        .worker_threads(8)
        .build()
        .unwrap();
    for j in 0..10 {
        rt.spawn(random_print(j));
    }
    /*
     * I'm being lazy here, and just delaying to let all tasks complete.
     * Using JoinHandle.await is much better. Also, comment out this line
     * and you will notice the program most likely ends before all 10 tasks
     * complete correctly!!! This is why JoinHandle.await is important!
     */
    sleep(Duration::from_millis(5000));
}
