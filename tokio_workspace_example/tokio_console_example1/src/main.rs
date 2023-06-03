// use futures::future;
//use std::error::Error;
use std::thread;
use std::time::Duration;
// use tokio::task::JoinHandle;
use tokio::time::sleep;
use tokio::task::Builder;

#[tokio::main]
// #[tokio::main(flavor = "current_thread")]
// #[tokio::main(flavor = "multi_thread", worker_threads = 10)]
async fn main() {
    console_subscriber::init(); // for tokio-console

    // 1. 論理CPU数の取得
    let cpus = num_cpus::get();
    println!("logical cores: {}", cpus);

    let mut handles = Vec::new();

    // 2. Task A: ノンブロッキング・スリープ
    let task_a = Builder::new().name("Task A").spawn(async {
        loop {
            println!("   =Task A sleeping... {:?}", thread::current().id());
            sleep(Duration::from_secs(1)).await; // ノンブロッキング・スリープ
            println!("   =Task A woke up.");
        }
    });
    handles.push(task_a);

    // 3. Task B-n: ブロッキング・スリープ
    for i in 0..cpus + 1 {
        let task_b = Builder::new()
            .name(&format!("Task B-{}", i))
            .spawn(async move {
                loop {
                    println!("#Task B-{} sleeping... {:?}", i, thread::current().id());
                    thread::sleep(Duration::from_secs(5)); // ブロッキング・スリープ
                    println!("#Task B-{} woke up.", i);
                    // sleep(Duration::from_nanos(1)).await;
                }
            });
        handles.push(task_b)
    }

    // future::join_all::<Vec<Result<JoinHandle<()>, dyn Error>>>(handles);
}
