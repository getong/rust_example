//! Host: 用 wasmer + wasmer-wasix 运行多线程 WASM 模块。
//!
//! wasmer 的多线程能力来自它的 WASIX 实现 (wasmer-wasix crate):
//!   - 原生支持 WASIX 线程 (wasix_32v1 命名空间, pthreads/fork/sockets 全套);
//!   - 同时兼容标准的 wasi-threads ABI —— 也就是 wasm32-wasip1-threads 编译出的模块
//!     ("wasi"."thread-spawn" 导入 + shared memory), 所以这里直接复用
//!     wasm_multi_process_example/wasm_threads_module 这个 guest。
//!
//! 对比 wasmtime 版 (wasm_multi_process_example/src/bin/wasi_threads_p1.rs):
//! 那边的 thread-spawn 是我们手写的 ~60 行; wasmer-wasix 把这套逻辑
//! (起线程 + 共享内存再实例化 + 调 wasi_thread_start) 全部内置了。

use std::path::PathBuf;

use anyhow::{Context, Result};
use wasmer::{Module, Store};
use wasmer_wasix::{WasiEnv, WasiError};

fn main() -> Result<()> {
  // 复用 wasmtime 例子的同一个多线程 guest 模块
  let wasm_path = std::env::args()
    .nth(1)
    .map(PathBuf::from)
    .unwrap_or_else(|| {
      PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../target/wasm32-wasip1-threads/release/wasm_threads_module.wasm")
    });

  // wasmer-wasix 用 tokio 做内部任务调度 (线程生成/IO), 先进入 runtime 上下文
  let tokio_rt = tokio::runtime::Runtime::new()?;
  let _guard = tokio_rt.enter();

  // Store::default(): 使用默认编译器后端 (cranelift)
  let mut store = Store::default();
  let module = Module::from_file(&store, &wasm_path)
    .map_err(anyhow::Error::from)
    .with_context(|| {
      format!(
        "加载 {wasm_path:?} 失败, 请先编译 guest:\n  cargo build -p wasm_threads_module --target \
         wasm32-wasip1-threads --release"
      )
    })?;

  println!("[host] wasmer + wasmer-wasix 启动多线程 wasm 模块 ...\n");

  // WasiEnv 一站式搞定: WASI 系统调用 + shared memory + thread-spawn。
  // instantiate() 会自动发现模块导入的 (shared) memory 并创建好;
  // guest 里每个 std::thread::spawn 都由 wasmer-wasix 映射成真正的线程
  let (instance, wasi_env) = WasiEnv::builder("wasm_threads_module")
    // 必须与加载 module 的是同一个 engine (spawn 新线程实例化时要用)
    .engine(store.engine().clone())
    // host -> guest 传参: 开几个线程、算多大规模
    .env("GUEST_THREADS", "4")
    .env("GUEST_N", "100000000")
    .instantiate(module, &mut store)?;

  let start = instance.exports.get_function("_start")?;
  let result = start.call(&mut store, &[]);
  wasi_env.on_exit(&mut store, None);

  match result {
    Ok(_) => {}
    // WASI 程序结束时以 proc_exit 退出, 表现为 WasiError::Exit
    Err(e) => match e.downcast::<WasiError>() {
      Ok(WasiError::Exit(code)) if code.is_success() => {}
      Ok(WasiError::Exit(code)) => anyhow::bail!("guest 以非零退出码结束: {code:?}"),
      Ok(other) => anyhow::bail!("wasi 错误: {other:?}"),
      Err(e) => return Err(e.into()),
    },
  }

  println!("\n[host] wasm 模块执行完毕 ✅");
  Ok(())
}
