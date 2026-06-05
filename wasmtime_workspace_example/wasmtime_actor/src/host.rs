use std::{
  collections::{HashMap, VecDeque},
  env,
  fmt::Write as _,
  path::{Path, PathBuf},
  process::Command,
  thread,
  time::Duration,
};

use anyhow::{Context, Result, anyhow, bail};
use uuid::Uuid;
use wasmtime::{
  Config, Engine, Store,
  component::{Component, HasSelf, Linker},
};
use wasmtime_wasi::{ResourceTable, WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};

use crate::bindings::{
  ActorWorld,
  demo::actor::host_actor::{
    self, ActorMsg, ActorMsgKind, ActorResponse, ActorState, ActorStateV1,
  },
};

const GUEST_TARGET: &str = "wasm32-wasip2";
const GUEST_WASM: &str = "wasmtime_actor.wasm";
const LOOP_SLEEP_MILLIS: i32 = 500;
const WASM_PROCESS_NAME: &str = "demo.actor.wasm";
const DEFAULT_UPGRADE_TICK: i32 = 6;

#[derive(Clone, Debug)]
struct WasmProcessRef {
  id: Uuid,
  name: String,
}

#[derive(Default)]
struct WasmProcessRegistry {
  by_id: HashMap<Uuid, WasmProcessRef>,
  by_name: HashMap<String, Uuid>,
}

impl WasmProcessRegistry {
  fn register(&mut self, name: impl Into<String>) -> Result<WasmProcessRef> {
    let name = name.into();
    if self.by_name.contains_key(&name) {
      bail!("wasm process name `{name}` is already registered");
    }

    let process = WasmProcessRef {
      id: Uuid::now_v7(),
      name,
    };
    self.by_name.insert(process.name.clone(), process.id);
    self.by_id.insert(process.id, process.clone());
    Ok(process)
  }

  fn whereis(&self, name: &str) -> Option<&WasmProcessRef> {
    self.by_name.get(name).and_then(|id| self.by_id.get(id))
  }
}

pub struct StoreState {
  host_handled: u64,
  process: WasmProcessRef,
  wasi: WasiCtx,
  table: ResourceTable,
}

impl WasiView for StoreState {
  fn ctx(&mut self) -> WasiCtxView<'_> {
    WasiCtxView {
      ctx: &mut self.wasi,
      table: &mut self.table,
    }
  }
}

impl host_actor::Host for StoreState {
  fn send_to_host(&mut self, msg: host_actor::GuestMessage) -> host_actor::ActorResponse {
    self.host_handled += 1;
    let reply =
      ((msg.tick as i64 + msg.last_host_reply as i64 + self.host_handled as i64) % 997) as i32;
    let message = {
      let cap = 48 + msg.payload.len();
      let mut buf = String::with_capacity(cap);
      write!(
        buf,
        "host processed wasm主动消息 `{}` after {} handled messages",
        msg.payload, msg.last_handled,
      )
      .unwrap();
      buf
    };
    let response = host_actor::ActorResponse {
      handled: self.host_handled,
      reply,
      message,
    };
    println!(
      "host actor: pid={} name={} tick={} payload={}, handled #{}, reply={reply}",
      self.process.id, self.process.name, msg.tick, msg.payload, self.host_handled
    );
    response
  }
}

struct LoadedActor {
  version: &'static str,
  component_path: PathBuf,
  instance: ActorWorld,
  state_schema: u32,
}

impl LoadedActor {
  fn load(
    engine: &Engine,
    linker: &Linker<StoreState>,
    store: &mut Store<StoreState>,
    version: &'static str,
    component_path: PathBuf,
  ) -> Result<Self> {
    let component = Component::from_file(engine, &component_path).map_err(|err| {
      anyhow!(
        "failed to load guest component {}: {err}",
        component_path.display()
      )
    })?;
    let instance = ActorWorld::instantiate(&mut *store, &component, linker).map_err(|err| {
      anyhow!(
        "failed to instantiate {version} guest actor component {}: {err}",
        component_path.display()
      )
    })?;
    let state_schema = instance
      .wasm_actor()
      .call_state_schema(store)
      .map_err(|err| anyhow!("failed to query {version} actor state schema: {err}"))?;

    Ok(Self {
      version,
      component_path,
      instance,
      state_schema,
    })
  }

  fn handle_call(
    &self,
    store: &mut Store<StoreState>,
    msgs: &[ActorMsg],
    state: &ActorState,
  ) -> Result<ActorState> {
    self
      .instance
      .wasm_actor()
      .call_handle_call(store, msgs, state)
      .map_err(|err| anyhow!("{} handle-call failed: {err}", self.version))
  }

  fn migrate_state(&self, store: &mut Store<StoreState>, state: &ActorState) -> Result<ActorState> {
    self
      .instance
      .wasm_actor()
      .call_migrate_state(store, state)
      .map_err(|err| anyhow!("{} migrate-state failed: {err}", self.version))
  }

  fn render_state(&self, store: &mut Store<StoreState>, state: &ActorState) -> Result<String> {
    self
      .instance
      .wasm_actor()
      .call_render_state(store, state)
      .map_err(|err| anyhow!("{} render-state failed: {err}", self.version))
  }
}

pub fn run() -> Result<()> {
  let max_ticks = max_ticks_from_env()?;
  let upgrade_tick = upgrade_tick_from_env()?;
  let guest_components = ensure_guest_components()?;

  let mut config = Config::new();
  config.wasm_component_model(true);
  let engine = Engine::new(&config)?;

  let mut linker = Linker::new(&engine);
  wasmtime_wasi::p2::add_to_linker_sync(&mut linker)?;
  host_actor::add_to_linker::<_, HasSelf<_>>(&mut linker, |state| state)?;

  let mut registry = WasmProcessRegistry::default();
  let process = registry.register(WASM_PROCESS_NAME)?;
  let registered = registry
    .whereis(WASM_PROCESS_NAME)
    .context("registered wasm process was not found by name")?;
  println!(
    "host registry: register name={} pid={} version=uuid-v7",
    registered.name, registered.id
  );
  println!(
    "host registry: whereis({}) -> pid={}",
    WASM_PROCESS_NAME, registered.id
  );

  let mut store = Store::new(
    &engine,
    StoreState {
      host_handled: 0,
      process: process.clone(),
      wasi: WasiCtxBuilder::new().inherit_stdio().build(),
      table: ResourceTable::new(),
    },
  );

  let mut actor = LoadedActor::load(
    &engine,
    &linker,
    &mut store,
    "guest-v1",
    guest_components.v1.clone(),
  )?;

  if max_ticks == 0 {
    println!(
      "host: driving wasm process {} ({}) through handle-call forever; soft-upgrade at tick \
       {upgrade_tick}; press Ctrl+C to stop",
      process.name, process.id
    );
  } else {
    println!(
      "host: driving wasm process {} ({}) for {max_ticks} ticks; soft-upgrade at tick \
       {upgrade_tick}",
      process.name, process.id
    );
  }
  println!(
    "host: loaded {} schema={} from {}",
    actor.version,
    actor.state_schema,
    actor.component_path.display()
  );

  let mut state = initial_actor_state();
  let mut upgraded = false;
  let mut host_messages = initial_host_messages();
  loop {
    if !upgraded && tick_of(&state) >= upgrade_tick {
      let next = LoadedActor::load(
        &engine,
        &linker,
        &mut store,
        "guest-v2",
        guest_components.v2.clone(),
      )?;
      let message = upgrade_actor(&mut store, &actor, next, &mut state)?;
      actor = message.actor;
      upgraded = true;
      println!("{}", message.summary);
    }

    let host_message = host_messages.pop_front();
    let pending_host_msgs = !host_messages.is_empty();

    let mut msgs = Vec::with_capacity(2);
    msgs.push(ActorMsg {
      kind: ActorMsgKind::Tick,
      host_message: None,
    });
    if let Some(host_msg) = host_message {
      msgs.push(ActorMsg {
        kind: ActorMsgKind::HostMessage,
        host_message: Some(host_msg),
      });
    }

    state = actor.handle_call(&mut store, &msgs, &state)?;

    if max_ticks > 0 && tick_of(&state) >= max_ticks {
      println!(
        "host: leaving verification loop after {} ticks with {} state schema {}",
        tick_of(&state),
        actor.version,
        actor.state_schema
      );
      break;
    }

    if !pending_host_msgs {
      thread::sleep(Duration::from_millis(LOOP_SLEEP_MILLIS as u64));
    }
  }

  let result = actor.render_state(&mut store, &state)?;
  println!("host: final shared state: {result}");
  println!(
    "host: StoreState for pid={} handled {} wasm主动 messages",
    store.data().process.id,
    store.data().host_handled
  );

  Ok(())
}

struct UpgradeMessage {
  actor: LoadedActor,
  summary: String,
}

fn upgrade_actor(
  store: &mut Store<StoreState>,
  current: &LoadedActor,
  next: LoadedActor,
  state: &mut ActorState,
) -> Result<UpgradeMessage> {
  if next.state_schema <= current.state_schema {
    bail!(
      "refusing actor upgrade {} schema {} -> {} schema {}",
      current.version,
      current.state_schema,
      next.version,
      next.state_schema,
    );
  }

  let mut shadow_state = next.migrate_state(store, state)?;
  match (&*state, &shadow_state) {
    (ActorState::V1(_), ActorState::V2(_)) => {}
    (before, after) => {
      bail!(
        "upgrade did not change actor-state variant: before={}, after={}",
        state_variant(before),
        state_variant(after),
      );
    }
  }

  shadow_state = next.handle_call(store, &[], &shadow_state)?;
  if !matches!(shadow_state, ActorState::V2(_)) {
    bail!(
      "new actor validation returned {} instead of v2 state",
      state_variant(&shadow_state)
    );
  }

  *state = shadow_state;

  let summary = format!(
    "host: soft upgrade {} schema {} -> {} schema {}; ActorState V1 migrated to V2",
    current.version, current.state_schema, next.version, next.state_schema
  );

  Ok(UpgradeMessage {
    actor: next,
    summary,
  })
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

fn tick_of(state: &ActorState) -> i32 {
  match state {
    ActorState::V1(state) => state.tick,
    ActorState::V2(state) => state.tick,
  }
}

fn state_variant(state: &ActorState) -> &'static str {
  match state {
    ActorState::V1(_) => "v1",
    ActorState::V2(_) => "v2",
  }
}

fn initial_host_messages() -> VecDeque<host_actor::HostMessage> {
  let mut messages = VecDeque::new();
  for sequence in 1u64 ..= 3 {
    let message = host_actor::HostMessage {
      sequence,
      payload: format!("host queued message {sequence}"),
    };
    println!(
      "host: queued message for wasm: seq={} payload={}",
      message.sequence, message.payload
    );
    messages.push_back(message);
  }
  messages
}

fn max_ticks_from_env() -> Result<i32> {
  let value = match env::var("WASMTIME_ACTOR_MAX_TICKS") {
    Ok(value) => value,
    Err(env::VarError::NotPresent) => return Ok(0),
    Err(err) => bail!("failed to read WASMTIME_ACTOR_MAX_TICKS: {err}"),
  };

  let max_ticks = value
    .parse::<i32>()
    .with_context(|| format!("WASMTIME_ACTOR_MAX_TICKS must be an integer, got `{value}`"))?;
  if max_ticks < 0 {
    bail!("WASMTIME_ACTOR_MAX_TICKS must be >= 0, got {max_ticks}");
  }

  Ok(max_ticks)
}

fn upgrade_tick_from_env() -> Result<i32> {
  let value = match env::var("WASMTIME_ACTOR_UPGRADE_TICK") {
    Ok(value) => value,
    Err(env::VarError::NotPresent) => return Ok(DEFAULT_UPGRADE_TICK),
    Err(err) => bail!("failed to read WASMTIME_ACTOR_UPGRADE_TICK: {err}"),
  };

  let upgrade_tick = value
    .parse::<i32>()
    .with_context(|| format!("WASMTIME_ACTOR_UPGRADE_TICK must be an integer, got `{value}`"))?;
  if upgrade_tick < 0 {
    bail!("WASMTIME_ACTOR_UPGRADE_TICK must be >= 0, got {upgrade_tick}");
  }

  Ok(upgrade_tick)
}

struct GuestComponents {
  v1: PathBuf,
  v2: PathBuf,
}

fn ensure_guest_components() -> Result<GuestComponents> {
  let package_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
  let workspace_dir = package_dir
    .parent()
    .context("failed to determine workspace root for wasmtime_actor")?;
  let guest_target_dir = workspace_dir.join("target").join("wasmtime-actor-guest");

  Ok(GuestComponents {
    v1: build_guest_component(package_dir, &guest_target_dir, "guest-v1")?,
    v2: build_guest_component(package_dir, &guest_target_dir, "guest-v2")?,
  })
}

fn build_guest_component(
  package_dir: &Path,
  guest_target_dir: &Path,
  feature: &'static str,
) -> Result<PathBuf> {
  let component_dir = guest_target_dir.join(feature);
  let guest_component = component_dir
    .join(GUEST_TARGET)
    .join("debug")
    .join(GUEST_WASM);

  let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
  let output = Command::new(cargo)
    .current_dir(package_dir)
    .arg("build")
    .arg("--lib")
    .arg("--target")
    .arg(GUEST_TARGET)
    .arg("--target-dir")
    .arg(&component_dir)
    .arg("--no-default-features")
    .arg("--features")
    .arg(feature)
    .output()
    .with_context(|| format!("failed to invoke cargo to build {feature} wasm guest component"))?;

  if !output.status.success() {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if stderr.contains("the `wasm32-wasip2` target may not be installed")
      || stderr.contains("can't find crate for `core`")
    {
      bail!(
        "failed to build `{GUEST_WASM}` for `{GUEST_TARGET}`.\ninstall the target first with \
         `rustup target add {GUEST_TARGET}` and rerun `cargo run -p wasmtime_actor --bin \
         wasmtime_actor`.\n\ncargo stderr:\n{stderr}"
      );
    }

    bail!(
      "failed to build {feature} guest component before launching the host actor.\nmanifest: \
       {}\nexpected output: {}\n\ncargo stdout:\n{stdout}\n\ncargo stderr:\n{stderr}",
      package_dir.join("Cargo.toml").display(),
      guest_component.display(),
    );
  }

  if !guest_component.is_file() {
    bail!(
      "cargo reported success, but the {feature} guest component was not produced at {}",
      guest_component.display(),
    );
  }

  Ok(guest_component)
}
