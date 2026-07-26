//! WASI p2 组件 guest: 导出 sum-range 纯计算函数。
//!
//! p2/组件模型里 guest 内部拿不到 std::thread (WASI 0.2 没有线程 API),
//! 并行的正确姿势是 host 侧 "每线程一个实例" —— 组件实例之间内存互相隔离,
//! 所以可以放心地在多个 OS 线程上同时跑, 不需要任何锁。

wit_bindgen::generate!({ world: "parallel-sum" });

struct ParallelSum;

impl Guest for ParallelSum {
  fn sum_range(lo: u64, hi: u64) -> u64 {
    println!("[guest] 实例收到任务: sum({lo}..={hi})");
    // black_box 防止 LLVM 把求和折叠成 n(n+1)/2 闭式公式, 保证真的在算
    (lo ..= hi).map(std::hint::black_box).sum()
  }
}

export!(ParallelSum);
