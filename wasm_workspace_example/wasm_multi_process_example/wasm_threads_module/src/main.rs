//! 运行在 wasm 里的多线程 guest 程序 (目标: wasm32-wasip1-threads)。
//!
//! 这里的 `std::thread::spawn` 最终会调用 host 提供的 `wasi::thread-spawn`,
//! host 会起一个真正的 OS 线程并共享同一块线性内存 (shared memory),
//! 所以 Arc / Mutex / Atomic / channel 这些标准库并发原语全部可用。

use std::{
  sync::{
    Arc, Mutex,
    atomic::{AtomicU32, Ordering},
    mpsc,
  },
  thread,
  time::Duration,
};

/// 所有线程共享的原子计数器 (位于 shared linear memory 中)
static FINISHED: AtomicU32 = AtomicU32::new(0);

fn main() {
  // host 通过 WASI env 把参数传进来
  let n_threads: u64 = std::env::var("GUEST_THREADS")
    .ok()
    .and_then(|v| v.parse().ok())
    .unwrap_or(4);
  let n: u64 = std::env::var("GUEST_N")
    .ok()
    .and_then(|v| v.parse().ok())
    .unwrap_or(10_000_000);

  println!("[guest:main] 启动, 准备用 {n_threads} 个线程并行计算 1..={n} 的和");

  // 1) Arc<Mutex<...>> 收集每个线程的部分和
  let partials = Arc::new(Mutex::new(vec![0u64; n_threads as usize]));
  // 2) mpsc channel 汇报进度
  let (tx, rx) = mpsc::channel::<String>();

  let chunk = n / n_threads;
  let mut handles = Vec::new();
  for i in 0 .. n_threads {
    let partials = Arc::clone(&partials);
    let tx = tx.clone();
    handles.push(thread::spawn(move || {
      let lo = i * chunk + 1;
      let hi = if i == n_threads - 1 {
        n
      } else {
        (i + 1) * chunk
      };
      let sum: u64 = (lo ..= hi).sum();

      partials.lock().unwrap()[i as usize] = sum;
      FINISHED.fetch_add(1, Ordering::SeqCst);
      tx.send(format!(
        "[guest:{:?}] 分片 {i}: sum({lo}..={hi}) = {sum}",
        thread::current().id()
      ))
      .unwrap();
      thread::sleep(Duration::from_millis(10)); // 展示线程真正并发存活
    }));
  }
  drop(tx);

  // channel 接收各线程的汇报 (乱序到达, 说明确实是并发执行)
  for msg in rx {
    println!("{msg}");
  }
  for h in handles {
    h.join().unwrap();
  }

  let total: u64 = partials.lock().unwrap().iter().sum();
  let expected = n * (n + 1) / 2;
  println!(
    "[guest:main] {} 个线程全部结束 (原子计数器 = {})",
    n_threads,
    FINISHED.load(Ordering::SeqCst)
  );
  println!("[guest:main] 并行求和结果 = {total}, 期望值 = {expected}");
  assert_eq!(total, expected);
  println!("[guest:main] ✅ 校验通过");
}
