use std::time::Instant;

// 由 build.rs 编译的 C++ shim(src/tbb_demo.cpp),内部调用 vcpkg 安装的 TBB
unsafe extern "C" {
  fn tbb_max_concurrency() -> i32;
  fn tbb_parallel_sum_sqrt(n: i64) -> f64;
}

fn main() {
  let n: i64 = 200_000_000;

  println!("TBB max concurrency: {}", unsafe { tbb_max_concurrency() });

  let t = Instant::now();
  let seq: f64 = (0 .. n).map(|i| (i as f64).sqrt()).sum();
  println!("Rust 顺序计算:      sum = {seq:.3}, 耗时 {:?}", t.elapsed());

  let t = Instant::now();
  let par = unsafe { tbb_parallel_sum_sqrt(n) };
  println!(
    "TBB parallel_reduce: sum = {par:.3}, 耗时 {:?}",
    t.elapsed()
  );
}
