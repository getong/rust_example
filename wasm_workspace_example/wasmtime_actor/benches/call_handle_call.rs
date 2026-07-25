use std::{
  env,
  path::{Path, PathBuf},
  process::Command,
  time::Duration,
};

use anyhow::{Context, Result, anyhow, bail};
use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use wasmtime::{
  Config, Engine, Store,
  component::{Component, HasSelf, Linker},
};
use wasmtime_wasi::{ResourceTable, WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};

wasmtime::component::bindgen!({
  path: "src/wit",
  world: "actor-world",
  require_store_data_send: true,
});

use demo::actor::host_actor::{
  self, ActorMsg, ActorMsgKind, ActorResponse, ActorState, ActorStateV1,
};

const GUEST_TARGET: &str = "wasm32-wasip2";
const GUEST_WASM: &str = "wasmtime_actor.wasm";

struct BenchStoreState {
  host_callbacks: u64,
  wasi: WasiCtx,
  table: ResourceTable,
}

impl WasiView for BenchStoreState {
  fn ctx(&mut self) -> WasiCtxView<'_> {
    WasiCtxView {
      ctx: &mut self.wasi,
      table: &mut self.table,
    }
  }
}

impl host_actor::Host for BenchStoreState {
  fn send_to_host(&mut self, msg: host_actor::GuestMessage) -> host_actor::ActorResponse {
    self.host_callbacks += 1;
    ActorResponse {
      handled: self.host_callbacks,
      reply: msg.tick,
      message: String::new(),
    }
  }
}

struct BenchActor {
  store: Store<BenchStoreState>,
  instance: ActorWorld,
}

impl BenchActor {
  fn new() -> Result<Self> {
    let guest_component = ensure_guest_component()?;

    let mut config = Config::new();
    config.wasm_component_model(true);
    let engine = Engine::new(&config)?;
    let component = Component::from_file(&engine, &guest_component).map_err(|err| {
      anyhow!(
        "failed to load guest component {}: {err}",
        guest_component.display()
      )
    })?;

    let mut linker = Linker::new(&engine);
    wasmtime_wasi::p2::add_to_linker_sync(&mut linker)?;
    host_actor::add_to_linker::<_, HasSelf<_>>(&mut linker, |state| state)?;

    let mut store = Store::new(
      &engine,
      BenchStoreState {
        host_callbacks: 0,
        wasi: WasiCtxBuilder::new().build(),
        table: ResourceTable::new(),
      },
    );
    let instance = ActorWorld::instantiate(&mut store, &component, &linker)
      .map_err(|err| anyhow!("failed to instantiate guest actor component: {err}"))?;

    let warmup_msgs = tick_messages(1);
    let warmup_state = initial_actor_state();
    instance
      .wasm_actor()
      .call_handle_call(&mut store, &warmup_msgs, &warmup_state)
      .map_err(|err| anyhow!("failed to warm up call_handle_call: {err}"))?;

    Ok(Self { store, instance })
  }

  fn call_handle_call(&mut self, msgs: &[ActorMsg], state: &ActorState) -> ActorState {
    self
      .instance
      .wasm_actor()
      .call_handle_call(&mut self.store, msgs, state)
      .expect("call_handle_call benchmark invocation should succeed")
  }
}

fn bench_call_handle_call(c: &mut Criterion) {
  let mut actor = BenchActor::new().expect("failed to initialize wasmtime actor benchmark");
  let tick_1 = tick_messages(1);
  let tick_5 = tick_messages(5);

  let mut group = c.benchmark_group("call_handle_call");

  group.throughput(Throughput::Elements(1));
  group.bench_function(BenchmarkId::new("tick_batch", 1), |b| {
    b.iter_batched(
      initial_actor_state,
      |state| {
        let next =
          actor.call_handle_call(std::hint::black_box(&tick_1), std::hint::black_box(&state));
        std::hint::black_box(next);
      },
      BatchSize::SmallInput,
    );
  });

  group.throughput(Throughput::Elements(5));
  group.bench_function(BenchmarkId::new("tick_batch", 5), |b| {
    b.iter_batched(
      initial_actor_state,
      |state| {
        let next =
          actor.call_handle_call(std::hint::black_box(&tick_5), std::hint::black_box(&state));
        std::hint::black_box(next);
      },
      BatchSize::SmallInput,
    );
  });

  group.finish();
}

fn tick_messages(count: usize) -> Vec<ActorMsg> {
  (0 .. count)
    .map(|_| ActorMsg {
      kind: ActorMsgKind::Tick,
      host_message: None,
    })
    .collect()
}

fn initial_actor_state() -> ActorState {
  ActorState::V1(ActorStateV1 {
    tick: 0,
    last_host_reply: 0,
    elapsed_since_push: 0,
    last_response: ActorResponse {
      handled: 0,
      reply: 0,
      message: String::new(),
    },
  })
}

fn ensure_guest_component() -> Result<PathBuf> {
  let package_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
  let workspace_dir = package_dir
    .parent()
    .context("failed to determine workspace root for wasmtime_actor")?;
  let guest_target_dir = workspace_dir
    .join("target")
    .join("wasmtime-actor-bench-guest");
  let guest_component = guest_target_dir
    .join(GUEST_TARGET)
    .join("release")
    .join(GUEST_WASM);

  let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
  let output = Command::new(cargo)
    .current_dir(package_dir)
    .arg("build")
    .arg("--lib")
    .arg("--release")
    .arg("--target")
    .arg(GUEST_TARGET)
    .arg("--target-dir")
    .arg(&guest_target_dir)
    .arg("--no-default-features")
    .arg("--features")
    .arg("guest-v1")
    .output()
    .context("failed to invoke cargo to build the wasm guest component")?;

  if !output.status.success() {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if stderr.contains("the `wasm32-wasip2` target may not be installed")
      || stderr.contains("can't find crate for `core`")
    {
      bail!(
        "failed to build `{GUEST_WASM}` for `{GUEST_TARGET}`.\ninstall the target first with \
         `rustup target add {GUEST_TARGET}` and rerun `cargo bench -p wasmtime_actor --bench \
         call_handle_call`.\n\ncargo stderr:\n{stderr}"
      );
    }

    bail!(
      "failed to build the guest component before benchmarking call_handle_call.\nmanifest: \
       {}\nexpected output: {}\n\ncargo stdout:\n{stdout}\n\ncargo stderr:\n{stderr}",
      package_dir.join("Cargo.toml").display(),
      guest_component.display(),
    );
  }

  if !guest_component.is_file() {
    bail!(
      "cargo reported success, but the guest component was not produced at {}",
      guest_component.display(),
    );
  }

  Ok(guest_component)
}

criterion_group! {
  name = benches;
  config = Criterion::default()
    .warm_up_time(Duration::from_secs(1))
    .measurement_time(Duration::from_secs(3))
    .sample_size(20);
  targets = bench_call_handle_call
}
criterion_main!(benches);
