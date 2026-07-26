//! Host (p2 版): 用 wasmtime 的组件模型 + wasmtime_wasi::p2 多线程并行调用 WASM 组件。
//!
//! 为什么 p2 的多线程长这样?
//!   - WASI 0.2 / 组件模型没有 guest 内线程: std::thread 在 wasm32-wasip2 上 不可用, wasi-threads
//!     提案已被标记为 legacy (只服务 p1), 线程的未来在 shared-everything-threads 提案 + WASI p3。
//!   - 所以 p2 时代的标准并行模式是 **host 侧 scatter-gather**: 每个 OS 线程持有一个独立的 Store +
//!     组件实例 (实例间内存隔离, 无数据竞争), host 把任务切片分发给各线程, 最后汇总。Spin
//!     等生产运行时就是这个模型。
//!
//! 对比: `src/bin/wasi_threads_p1.rs` 是 guest 内 std::thread 的 p1 版本。

use std::{path::PathBuf, time::Instant};

use anyhow::{Context, Result};
use wasmtime::{
  Config, Engine, Store,
  component::{Component, InstancePre, Linker, ResourceTable},
};
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};

/// 每个 Store (= 每个线程各一个) 的宿主状态: p2 的 WasiCtx + 资源表
struct Ctx {
  wasi: WasiCtx,
  table: ResourceTable,
}

/// p2 的挂载方式: 实现 WasiView, 把 WasiCtx/ResourceTable 暴露给 wasmtime-wasi
impl WasiView for Ctx {
  fn ctx(&mut self) -> WasiCtxView<'_> {
    WasiCtxView {
      ctx: &mut self.wasi,
      table: &mut self.table,
    }
  }
}

fn new_store(engine: &Engine) -> Store<Ctx> {
  Store::new(
    engine,
    Ctx {
      wasi: WasiCtxBuilder::new().inherit_stdio().build(),
      table: ResourceTable::new(),
    },
  )
}

/// 在当前线程上: 新建 Store -> 实例化组件 -> 调用导出的 sum-range
fn call_sum_range(pre: &InstancePre<Ctx>, lo: u64, hi: u64) -> Result<u64> {
  let mut store = new_store(pre.engine());
  let instance = pre.instantiate(&mut store)?;
  let func = instance.get_typed_func::<(u64, u64), (u64,)>(&mut store, "sum-range")?;
  let (sum,) = func.call(&mut store, (lo, hi))?;
  Ok(sum)
}

fn main() -> Result<()> {
  // 组件路径: 默认取 workspace target 目录, 也可用第一个命令行参数指定
  let wasm_path = std::env::args()
    .nth(1)
    .map(PathBuf::from)
    .unwrap_or_else(|| {
      PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../target/wasm32-wasip2/release/wasm_p2_component.wasm")
    });

  let mut config = Config::new();
  config.wasm_component_model(true); // 47 默认已开启, 这里写出来表意
  let engine = Engine::new(&config)?;

  let component = Component::from_file(&engine, &wasm_path)
    .map_err(anyhow::Error::from)
    .with_context(|| {
      format!(
        "加载 {wasm_path:?} 失败, 请先编译 guest 组件:\n  cargo build -p wasm_p2_component \
         --target wasm32-wasip2 --release"
      )
    })?;

  // 挂上 WASI 0.2 (wasi:cli/io/clocks/filesystem/... 全套 p2 接口)
  let mut linker: Linker<Ctx> = Linker::new(&engine);
  wasmtime_wasi::p2::add_to_linker_sync(&mut linker)?;

  // InstancePre: 提前完成链接/类型检查, Send + Sync, 可安全共享给多个线程,
  // 每个线程用它快速实例化出自己的私有实例
  let pre = linker.instantiate_pre(&component)?;

  let n_threads: u64 = std::thread::available_parallelism()?.get().min(8) as u64;
  const N: u64 = 400_000_000;
  let expected = N * (N + 1) / 2;

  // ---- 基线: 单线程, 1 个实例算整个区间 ----
  println!("[host] 单线程基线: 1 个实例计算 sum(1..={N})");
  let t0 = Instant::now();
  let seq = call_sum_range(&pre, 1, N)?;
  let seq_cost = t0.elapsed();
  println!("[host] 结果 = {seq}, 耗时 {seq_cost:?}\n");

  // ---- 并行: n 个 OS 线程, 每线程一个独立实例, 各算一个分片 ----
  println!("[host] 并行: {n_threads} 个 OS 线程 x 各自独立的组件实例");
  let t1 = Instant::now();
  let chunk = N / n_threads;
  let total: u64 = std::thread::scope(|s| {
    let handles: Vec<_> = (0 .. n_threads)
      .map(|i| {
        let pre = &pre;
        s.spawn(move || {
          let lo = i * chunk + 1;
          let hi = if i == n_threads - 1 {
            N
          } else {
            (i + 1) * chunk
          };
          let sum = call_sum_range(pre, lo, hi).expect("wasm 调用失败");
          println!(
            "[host:{:?}] 分片 {i}: sum({lo}..={hi}) = {sum}",
            std::thread::current().id()
          );
          sum
        })
      })
      .collect();
    handles.into_iter().map(|h| h.join().unwrap()).sum()
  });
  let par_cost = t1.elapsed();

  println!("\n[host] 并行结果 = {total}, 期望值 = {expected}");
  assert_eq!(total, expected);
  assert_eq!(seq, expected);
  println!(
    "[host] 单线程 {seq_cost:?} vs {n_threads} 线程 {par_cost:?}, 加速比 ≈ {:.2}x ✅",
    seq_cost.as_secs_f64() / par_cost.as_secs_f64()
  );
  Ok(())
}
