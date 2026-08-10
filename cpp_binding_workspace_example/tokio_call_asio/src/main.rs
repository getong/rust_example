//! tokio 调用 C++ asio —— asio-call-tokio 参考项目的镜像方向。
//!
//! 协程无法直接跨语言边界，所以 FFI 表面是 completion-callback 形状：
//! Rust 侧每个 async 包装函数创建一个 tokio oneshot channel，把发送端
//! Box 成不透明指针 `ctx` 交给 C++；`extern "C++"` 函数把一个 asio 协程
//! 扔上后台 io_context 线程后立即返回；协程完成后从 asio 线程回调
//! `extern "Rust"` 的 complete_*，经 oneshot 把结果送回，唤醒正在
//! `.await` 的 tokio 任务。对调用者来说，一个普通的 `.await`
//! 就在等 C++ asio 协程。

use std::time::{Duration, Instant};

use tokio::sync::oneshot;

#[cxx::bridge(namespace = "asio_ffi")]
mod ffi {
  unsafe extern "C++" {
    include!("src/api.h");

    /// 在 asio 的 steady_timer 上等 delay_ms，然后把 lhs + rhs
    /// 经 complete_add(ctx, sum) 从 asio 线程送回。
    fn sleep_then_add(lhs: i32, rhs: i32, delay_ms: u64, ctx: usize);

    /// 模拟异步获取：等 delay_ms 后把问候语经 complete_greet 送回。
    fn fetch_greeting(name: &str, delay_ms: u64, ctx: usize);

    /// 撤掉 io_context 的 work guard 并 join 后台线程。
    fn shutdown();
  }

  extern "Rust" {
    /// asio 协程完成后从 asio 线程调用，把和送回 ctx 里的 oneshot。
    fn complete_add(ctx: usize, value: i32);

    /// 同上，送回问候语。
    fn complete_greet(ctx: usize, value: String);
  }
}

/// 把 oneshot 发送端打包成跨 FFI 的不透明指针。
/// 约定：每个 ctx 恰好被对应的 complete_* 回调消费一次。
fn park<T>(tx: oneshot::Sender<T>) -> usize {
  Box::into_raw(Box::new(tx)) as usize
}

/// park 的逆操作：拿回发送端并送出结果。
fn resume<T>(ctx: usize, value: T) {
  // SAFETY: ctx 由 park::<T> 创建，C++ 侧只是原样带回，
  // 且每个 ctx 只会被回调一次，因此这里独占所有权。
  let tx = unsafe { Box::from_raw(ctx as *mut oneshot::Sender<T>) };
  // 接收端可能已被丢弃（例如调用方取消了 await），忽略即可
  let _ = tx.send(value);
}

fn complete_add(ctx: usize, value: i32) {
  resume(ctx, value);
}

fn complete_greet(ctx: usize, value: String) {
  resume(ctx, value);
}

/// 一个 Rust future，实际的等待发生在 C++ asio 协程里。
async fn sleep_then_add(lhs: i32, rhs: i32, delay: Duration) -> i32 {
  let (tx, rx) = oneshot::channel();
  ffi::sleep_then_add(lhs, rhs, delay.as_millis() as u64, park(tx));
  rx.await.expect("asio dropped the completion callback")
}

/// 同上：await 一个在 asio 线程上生成问候语的 C++ 协程。
async fn fetch_greeting(name: &str, delay: Duration) -> String {
  let (tx, rx) = oneshot::channel();
  ffi::fetch_greeting(name, delay.as_millis() as u64, park(tx));
  rx.await.expect("asio dropped the completion callback")
}

#[tokio::main]
async fn main() {
  let started = Instant::now();
  let elapsed = move || started.elapsed().as_millis();

  println!(
    "[rust {:>4}ms] tokio task on {:?}, awaiting asio...",
    elapsed(),
    std::thread::current().id()
  );

  // 一次 await:tokio 任务在此挂起,asio 在自己的后台线程上等 200ms,
  // 结果经 oneshot 送回后 tokio 恢复本任务。
  let sum = sleep_then_add(40, 2, Duration::from_millis(200)).await;
  println!(
    "[rust {:>4}ms] c++ says 40 + 2 = {sum} (back on {:?})",
    elapsed(),
    std::thread::current().id()
  );

  // 并发 await 两个 asio 协程:总耗时约 300ms,而不是 550ms。
  let (next_sum, greeting) = tokio::join!(
    sleep_then_add(sum, 58, Duration::from_millis(300)),
    fetch_greeting("tokio_call_asio", Duration::from_millis(250)),
  );
  println!(
    "[rust {:>4}ms] concurrent awaits done: {sum} + 58 = {next_sum}",
    elapsed()
  );
  println!("[rust {:>4}ms] {greeting}", elapsed());

  // 干净关闭:撤掉 work guard,join io_context 线程
  ffi::shutdown();
}
