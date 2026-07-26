//! Host: 用 wasmtime 运行一个多线程 (wasi-threads) 的 WASM 模块。
//!
//! wasm32-wasip1-threads 编译出的模块有三个关键点:
//!   1. 从 "env" 导入一块 **shared memory** —— 所有线程共享同一块线性内存;
//!   2. 导入 "wasi" 模块的 "thread-spawn" 函数 —— guest 里 std::thread::spawn 最终会走到这里, 由
//!      host 负责真正创建线程;
//!   3. 导出 "wasi_thread_start(tid, arg)" —— 新线程的 wasm 侧入口。
//!
//! 官方的 wasmtime-wasi-threads 胶水 crate 在 wasmtime 37 之后已被移除,
//! 但它的核心逻辑很短, 这里直接手写 (~60 行): 每次 guest 调 thread-spawn,
//! host 就起一个 OS 线程, 用同一个 InstancePre + 同一块 shared memory
//! 重新实例化一次模块 (每个线程一个独立的 Store/栈, 但内存共享),
//! 然后调用 wasi_thread_start(tid, arg)。

use std::{
  path::PathBuf,
  sync::{
    Arc,
    atomic::{AtomicI32, Ordering},
  },
};

use anyhow::{Context, Result};
use wasmtime::{Caller, Config, Engine, InstancePre, Linker, Module, SharedMemory, Store};
use wasmtime_wasi::{WasiCtxBuilder, p1::WasiP1Ctx};

/// 每个 Store (= 每个线程) 的宿主状态
struct Host {
  wasi: WasiP1Ctx,
  /// 线程生成器, 主实例和所有子线程实例共享同一个
  spawner: Option<Arc<ThreadSpawner>>,
}

fn new_wasi_ctx() -> WasiP1Ctx {
  WasiCtxBuilder::new()
    .inherit_stdio()
    // host -> guest 传参: 控制 guest 里开几个线程、算多大规模
    .env("GUEST_THREADS", "4")
    .env("GUEST_N", "100000000")
    .build_p1()
}

/// wasi-threads 的核心: 负责 "thread-spawn" -> 真 OS 线程 + 模块再实例化
struct ThreadSpawner {
  instance_pre: InstancePre<Host>,
  next_tid: AtomicI32,
}

impl ThreadSpawner {
  /// 对应 guest 导入的 `wasi::thread-spawn(start_arg) -> tid`
  fn spawn(self: &Arc<Self>, start_arg: i32) -> Result<i32> {
    let tid = self.next_tid.fetch_add(1, Ordering::Relaxed);
    let spawner = Arc::clone(self);

    std::thread::Builder::new()
      .name(format!("wasi-thread-{tid}"))
      .spawn(move || {
        // 新线程: 独立的 Store, 但 InstancePre 里的 shared memory 是同一块
        let engine = spawner.instance_pre.module().engine().clone();
        let mut store = Store::new(
          &engine,
          Host {
            wasi: new_wasi_ctx(),
            spawner: Some(Arc::clone(&spawner)), // 支持线程里再开线程
          },
        );
        let instance = spawner
          .instance_pre
          .instantiate(&mut store)
          .expect("在新线程中实例化模块失败");
        let entry = instance
          .get_typed_func::<(i32, i32), ()>(&mut store, "wasi_thread_start")
          .expect("模块缺少 wasi_thread_start 导出");
        println!(
          "[host] OS 线程 {:?} 启动, 调用 wasi_thread_start(tid={tid})",
          std::thread::current().id()
        );
        if let Err(e) = entry.call(&mut store, (tid, start_arg)) {
          eprintln!("[host] wasi-thread-{tid} trap: {e:?}");
        }
      })?;
    Ok(tid)
  }
}

fn main() -> Result<()> {
  // wasm 模块路径: 默认取 workspace target 目录, 也可以用第一个命令行参数指定
  let wasm_path = std::env::args()
    .nth(1)
    .map(PathBuf::from)
    .unwrap_or_else(|| {
      PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../target/wasm32-wasip1-threads/release/wasm_threads_module.wasm")
    });

  // 1. 打开 threads 提案支持 (shared memory + 原子指令)
  let mut config = Config::new();
  config.wasm_threads(true); // 启用 threads 提案 (原子指令等)
  config.shared_memory(true); // 允许创建 shared memory (wasmtime 47+ 需单独打开)
  let engine = Engine::new(&config)?;

  let module = Module::from_file(&engine, &wasm_path)
    .map_err(anyhow::Error::from)
    .with_context(|| {
      format!(
        "加载 {wasm_path:?} 失败, 请先编译 guest:\n  cargo build -p wasm_threads_module --target \
         wasm32-wasip1-threads --release"
      )
    })?;

  let mut linker: Linker<Host> = Linker::new(&engine);

  // 2. 挂上 WASI preview1 (println/env/clock 等都靠它)
  wasmtime_wasi::p1::add_to_linker_sync(&mut linker, |h: &mut Host| &mut h.wasi)?;

  // 3. 按模块声明的类型创建 shared memory, 提供给 "env"."memory" 导入。
  //    这块内存被主实例和所有线程实例共享 —— 这就是 wasm 多线程共享状态的基础
  let mem_ty = module
    .imports()
    .find(|i| i.module() == "env" && i.name() == "memory")
    .and_then(|i| i.ty().memory().cloned())
    .context("模块没有导入 env.memory, 请确认用 wasm32-wasip1-threads 目标编译")?;
  let shared_memory = SharedMemory::new(&engine, mem_ty)?;
  println!(
    "[host] 创建 shared memory: min={} pages, max={:?} pages",
    shared_memory.ty().minimum(),
    shared_memory.ty().maximum()
  );

  // 4. 提供 "wasi"."thread-spawn" 宿主函数
  linker.func_wrap(
    "wasi",
    "thread-spawn",
    |caller: Caller<'_, Host>, start_arg: i32| -> i32 {
      let spawner = caller
        .data()
        .spawner
        .as_ref()
        .expect("spawner 未初始化")
        .clone();
      match spawner.spawn(start_arg) {
        Ok(tid) => tid,
        Err(e) => {
          eprintln!("[host] thread-spawn 失败: {e:?}");
          -1 // 负数 = 失败, guest 的 std 会把它转成 spawn 错误
        }
      }
    },
  )?;

  // 5. 实例化主实例并运行 _start (即 guest 的 main)
  let mut store = Store::new(
    &engine,
    Host {
      wasi: new_wasi_ctx(),
      spawner: None,
    },
  );
  linker.define(&store, "env", "memory", shared_memory.clone())?;

  let instance_pre = linker.instantiate_pre(&module)?;
  store.data_mut().spawner = Some(Arc::new(ThreadSpawner {
    instance_pre: instance_pre.clone(),
    next_tid: AtomicI32::new(1),
  }));

  let instance = instance_pre.instantiate(&mut store)?;
  let start = instance.get_typed_func::<(), ()>(&mut store, "_start")?;

  println!("[host] 调用 wasm 的 _start ...\n");
  match start.call(&mut store, ()) {
    Ok(()) => {}
    // WASI 程序正常结束时会以 proc_exit 退出, 表现为 I32Exit "错误"
    Err(e) => match e.downcast_ref::<wasmtime_wasi::I32Exit>() {
      Some(exit) if exit.0 == 0 => {}
      Some(exit) => anyhow::bail!("guest 以非零退出码结束: {}", exit.0),
      None => return Err(e.into()),
    },
  }

  println!("\n[host] wasm 模块执行完毕 ✅");
  Ok(())
}
