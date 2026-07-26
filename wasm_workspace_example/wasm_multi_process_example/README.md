# wasm 多线程示例 (wasmtime: p2 组件版 + p1 wasi-threads 版)

同一个主题的两种实现，展示 WASM 多线程的现状：

| | p2 组件版 (默认) | p1 wasi-threads 版 |
|---|---|---|
| 入口 | `src/main.rs` | `src/bin/wasi_threads_p1.rs` |
| guest | `wasm_p2_component/` (wasm32-wasip2 组件) | `wasm_threads_module/` (wasm32-wasip1-threads) |
| 线程在哪 | **host 侧**: 每个 OS 线程一个独立实例 (scatter-gather) | **guest 内**: `std::thread` 直接可用 |
| WASI API | `wasmtime_wasi::p2` + `WasiView` + 组件模型 | `wasmtime_wasi::p1` + 手写 `wasi::thread-spawn` |
| 内存 | 实例间隔离, 无锁无竞争 | 全部线程共享一块 shared memory |

## 为什么 p2 不能在 guest 里 `std::thread`?

这是 2026 年的客观现状, 不是本例偷懒:

- **WASI 0.2 / 组件模型没有线程 API**。`wasm32-wasip2` 目标下
  `std::thread::spawn` 直接返回错误 (rustc 平台文档明确说明);
- **wasi-threads 提案已是 legacy**, 2023 年 8 月起官方转向
  [shared-everything-threads](https://github.com/WebAssembly/shared-everything-threads),
  它只保留给 p1 (`wasm32-wasip1-threads` 目标)。wasmtime 37 起连
  `wasmtime-wasi-threads`/`wasi-common` crate 和 CLI 的 `-S threads` 都移除了;
- **guest 内线程的未来在 WASI p3**: shared-everything-threads + `wasm32-wasip3`
  目标 (rustc 文档: `std::thread` 支持将落在 wasip3), wasmtime 中该提案
  目前默认关闭、仍在实现中。

所以 p2 时代的标准并行模式是 **host 侧多线程 + 每线程一个组件实例**
(Spin 等生产运行时同款): `InstancePre` 是 `Send + Sync`, 分发给 N 个 OS
线程, 各自 `Store` + 实例、内存彼此隔离, 天然无数据竞争。

## 运行

```bash
# 一次性安装目标
rustup target add wasm32-wasip2 wasm32-wasip1-threads

# ---- p2 组件版 (默认) ----
cargo build -p wasm_p2_component --target wasm32-wasip2 --release
cargo run -p wasm_multi_process_example

# ---- p1 wasi-threads 版 ----
cargo build -p wasm_threads_module --target wasm32-wasip1-threads --release
cargo run -p wasm_multi_process_example --bin wasi_threads_p1
```

## p2 版要点 (src/main.rs)

- guest 用 `wasm32-wasip2` 编译, **直接产出组件**, WIT 世界导出
  `sum-range: func(lo: u64, hi: u64) -> u64` (wit-bindgen 0.60);
- host 状态实现 `WasiView` (p2 的挂载协议), `wasmtime_wasi::p2::add_to_linker_sync`
  一行挂上全套 WASI 0.2 接口;
- `linker.instantiate_pre(&component)` 得到可跨线程共享的 `InstancePre`,
  `std::thread::scope` 里每个线程实例化自己的组件算一个分片;
- guest 用 `black_box` 防止求和被折叠成闭式公式, 实测 8 线程加速比约 7x。

实测输出 (Apple Silicon, 8 线程):

```
[host] 单线程基线: 1 个实例计算 sum(1..=400000000)
[host] 结果 = 80000000200000000, 耗时 93.49ms
[host] 并行: 8 个 OS 线程 x 各自独立的组件实例
...
[host] 单线程 93.49ms vs 8 线程 13.46ms, 加速比 ≈ 6.94x ✅
```

## p1 版要点 (src/bin/wasi_threads_p1.rs)

`wasm32-wasip1-threads` 模块从 `env` 导入 shared memory、导入
`wasi::thread-spawn`、导出 `wasi_thread_start`。host 手写 thread-spawn
(官方胶水 crate 已随 wasmtime 37 移除): 每次被调用就起一个 OS 线程,
用同一个 `InstancePre` + 同一块 shared memory 再实例化一次模块并调用
`wasi_thread_start(tid, arg)`。guest 里 `std::thread` / `Arc<Mutex>` /
`AtomicU32` / `mpsc::channel` 全部开箱即用。
