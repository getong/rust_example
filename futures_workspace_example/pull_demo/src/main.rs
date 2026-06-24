use std::{
  future::Future,
  pin::Pin,
  sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
  },
  task::{Context, Poll, RawWakerVTable, Waker},
  thread,
  time::{Duration, Instant},
};

// ==========================================
// 1. 自定义 Future：一个简单的定时器
// ==========================================
struct TimerFuture {
  shared_state: Arc<Mutex<SharedState>>,
}

struct SharedState {
  completed: bool,
  waker: Option<Waker>,
}

impl Future for TimerFuture {
  type Output = String;

  // 每次被 Executor 轮询时执行
  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    let mut shared_state = self.shared_state.lock().unwrap();

    if shared_state.completed {
      println!("[Future] 状态就绪！返回 Ready。");
      Poll::Ready("任务成功完成！".to_string())
    } else {
      println!("[Future] 还没准备好，保存 Waker 并返回 Pending。");
      // 克隆当前上下文的 waker，保存到共享状态中
      // 当后台线程完成等待时，会通过这个 waker 唤醒 Executor
      shared_state.waker = Some(cx.waker().clone());
      Poll::Pending
    }
  }
}

impl TimerFuture {
  // 创建一个新的定时器 Future，并启动后台线程模拟异步 I/O
  fn new(duration: Duration) -> Self {
    let shared_state = Arc::new(Mutex::new(SharedState {
      completed: false,
      waker: None,
    }));

    let thread_shared_state = shared_state.clone();

    // 衍生一个后台线程，模拟操作系统的异步事件（如网络或定时器）
    thread::spawn(move || {
      println!("[后台线程] 开始倒计时...");
      thread::sleep(duration);

      let mut state = thread_shared_state.lock().unwrap();
      state.completed = true;

      // 如果 Executor 之前来轮询过并留下了 waker，就唤醒它
      if let Some(waker) = state.waker.take() {
        println!("[后台线程] 时间到！调用 waker.wake() 触发通知。");
        waker.wake();
      }
    });

    TimerFuture { shared_state }
  }
}

// ==========================================
// 2. 模拟一个极简的 Executor (执行器)
// ==========================================
// 生产环境的 Executor 会使用通道（Channel）和任务队列。
// 为了绝对简化，我们用一个原子变量来充当“是否需要重新轮询”的信号。
struct MiniExecutor;

impl MiniExecutor {
  // 阻塞当前线程，直到 Future 运行结束
  fn block_on<F: Future>(mut future: F) -> F::Output {
    // 将 Future 固定在内存中（Pin）
    let mut future = unsafe { Pin::new_unchecked(&mut future) };

    // 创建一个简单的信号，用来标识任务是否被唤醒
    let ready_signal = Arc::new(AtomicBool::new(true));

    // 利用标准库的 RawWaker 手动构建一个 Waker
    // 当 waker.wake() 被调用时，将 ready_signal 设为 true
    let raw_waker = MyWaker::create_raw_waker(ready_signal.clone());
    let waker = unsafe { Waker::from_raw(raw_waker) };
    let mut cx = Context::from_waker(&waker);

    loop {
      // 如果收到唤醒信号，则再次轮询
      if ready_signal.swap(false, Ordering::Relaxed) {
        println!("[Executor] 收到执行信号，开始调用 poll()...");
        match future.as_mut().poll(&mut cx) {
          Poll::Ready(result) => return result,
          Poll::Pending => {
            println!("[Executor] 轮询结束，进入阻塞等待状态...");
          }
        }
      }
      // 简单让出 CPU 轮片，避免死循环空转
      thread::sleep(Duration::from_millis(50));
    }
  }
}

// ==========================================
// 3. 手动构建 Waker 的底层胶水代码
// ==========================================
struct MyWaker;
impl MyWaker {
  fn create_raw_waker(signal: Arc<AtomicBool>) -> std::task::RawWaker {
    let raw = Arc::into_raw(signal) as *const ();
    std::task::RawWaker::new(raw, &VTABLE)
  }
}

// 虚函数表：定义 Waker 被克隆、唤醒、释放时的底层行为
const VTABLE: RawWakerVTable = RawWakerVTable::new(
  // clone
  |data| unsafe {
    Arc::increment_strong_count(data);
    std::task::RawWaker::new(data, &VTABLE)
  },
  // wake (消耗 self)
  |data| unsafe {
    let signal = Arc::from_raw(data as *const AtomicBool);
    signal.store(true, Ordering::Relaxed);
  },
  // wake_by_ref (不消耗 self)
  |data| unsafe {
    let signal = &*(data as *const AtomicBool);
    signal.store(true, Ordering::Relaxed);
  },
  // drop
  |data| unsafe {
    Arc::from_raw(data as *const AtomicBool);
  },
);

// ==========================================
// 4. 主函数测试
// ==========================================
fn main() {
  println!("--- 程序开始 ---");
  let start_time = Instant::now();

  // 创建一个需要等待 1 秒的异步任务
  let my_future = TimerFuture::new(Duration::from_secs(1));

  // 使用我们写好的执行器去跑这个 Future
  let result = MiniExecutor::block_on(my_future);

  println!("[Main] 最终收到结果: {}", result);
  println!("--- 程序结束，耗时: {:?}", start_time.elapsed());
}
