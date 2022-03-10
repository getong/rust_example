use chrono::Local;
use std::thread;
use tokio::{self, runtime::Runtime, time};

fn now() -> String {
    Local::now().format("%F %T").to_string()
}

fn main() {
    let rt = Runtime::new().unwrap();
    // 一个运行5秒的blocking thread
    // 删除rt时，该任务将继续运行，直到自己终止
    rt.spawn_blocking(|| {
        thread::sleep(std::time::Duration::from_secs(5));
        println!("blocking thread task over: {}", now());
    });

    // 进入runtime，并生成一个运行3秒的异步任务，
    // 删除rt时，该任务直接被终止
    let _guard = rt.enter();
    rt.spawn(async {
        time::sleep(time::Duration::from_secs(3)).await;
        println!("worker thread task over 1: {}", now());
    });

    // 进入runtime，并生成一个运行4秒的阻塞整个线程的任务
    // 删除rt时，该任务继续运行，直到自己终止
    rt.spawn(async {
        std::thread::sleep(std::time::Duration::from_secs(4));
        println!("worker thread task over 2: {}", now());
    });

    // 先让所有任务运行起来
    std::thread::sleep(std::time::Duration::from_millis(3));

    // 删除runtime句柄，将直接移除那个3秒的异步任务，
    // 且阻塞5秒，直到所有已经阻塞的thread完成
    drop(rt);
    println!("runtime droped: {}", now());
}
