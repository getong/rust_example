# wasmer 多线程示例 (wasmer + wasmer-wasix)

用 **wasmer** 运行多线程 WASM 模块。wasmer 的多线程能力来自它的
[WASIX](https://wasix.org) 扩展 (`wasmer-wasix` crate)：

- 原生支持 WASIX 线程（`wasix_32v1` 命名空间，pthreads/fork/sockets 全套）；
- 同时**兼容标准 wasi-threads ABI**（`wasi::thread-spawn` + shared memory），
  即 `wasm32-wasip1-threads` 目标编译出的模块可以直接跑——
  本例复用了 `wasm_multi_process_example/wasm_threads_module` 这个 guest，
  guest 里 `std::thread` / `Arc<Mutex>` / atomics / channel 开箱即用。

对比 wasmtime 版（`wasm_multi_process_example/src/bin/wasi_threads_p1.rs`）：
那边的 `thread-spawn` 是手写的 ~60 行；wasmer-wasix 把这套逻辑
（起线程 + 共享内存再实例化 + 调 `wasi_thread_start`）全部内置，
host 侧只需 `WasiEnv::builder(...).instantiate(...)` 再调 `_start`。

## 运行

```bash
# guest 与 wasmtime 例子共用 (一次性)
rustup target add wasm32-wasip1-threads
cargo build -p wasm_threads_module --target wasm32-wasip1-threads --release

cargo run -p wasmer_threads_example
```

## 版本要点

- `wasmer-wasix` 对 `wasmer` 是精确版本锁定：`0.702.0` ↔ `=7.2.0`，必须配对；
- builder 必须 `.engine(store.engine().clone())`——spawn 新线程时
  wasmer-wasix 要用同一个 engine 重新实例化模块；
- wasmer-wasix 内部用 tokio 调度，host 需先 `tokio_rt.enter()` 进入上下文；
- guest 正常结束表现为 `WasiError::Exit(0)`，需要在 `_start` 的返回值里处理。

## 示例输出

```
[host] wasmer + wasmer-wasix 启动多线程 wasm 模块 ...

[guest:main] 启动, 准备用 4 个线程并行计算 1..=100000000 的和
[guest:ThreadId(2)] 分片 0: sum(1..=25000000) = 312500012500000
[guest:ThreadId(3)] 分片 1: sum(25000001..=50000000) = 937500012500000
[guest:ThreadId(4)] 分片 2: sum(50000001..=75000000) = 1562500012500000
[guest:ThreadId(5)] 分片 3: sum(75000001..=100000000) = 2187500012500000
[guest:main] 4 个线程全部结束 (原子计数器 = 4)
[guest:main] 并行求和结果 = 5000000050000000, 期望值 = 5000000050000000
[guest:main] ✅ 校验通过

[host] wasm 模块执行完毕 ✅
```
