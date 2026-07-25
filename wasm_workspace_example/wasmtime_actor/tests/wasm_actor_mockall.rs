use std::{
  env,
  path::{Path, PathBuf},
  process::Command,
};

use anyhow::{Context, Result, anyhow, bail};
use mockall::automock;
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

#[automock]
trait HostGateway {
  fn send_to_host(&mut self, msg: host_actor::GuestMessage) -> ActorResponse;
}

struct MockStoreState {
  host: MockHostGateway,
  wasi: WasiCtx,
  table: ResourceTable,
}

impl WasiView for MockStoreState {
  fn ctx(&mut self) -> WasiCtxView<'_> {
    WasiCtxView {
      ctx: &mut self.wasi,
      table: &mut self.table,
    }
  }
}

impl host_actor::Host for MockStoreState {
  fn send_to_host(&mut self, msg: host_actor::GuestMessage) -> host_actor::ActorResponse {
    self.host.send_to_host(msg)
  }
}

struct MockedActor {
  store: Store<MockStoreState>,
  instance: ActorWorld,
}

impl MockedActor {
  fn new(feature: &'static str, host: MockHostGateway) -> Result<Self> {
    let guest_component = ensure_guest_component(feature)?;

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
      MockStoreState {
        host,
        wasi: WasiCtxBuilder::new().build(),
        table: ResourceTable::new(),
      },
    );
    let instance = ActorWorld::instantiate(&mut store, &component, &linker)
      .map_err(|err| anyhow!("failed to instantiate guest actor component: {err}"))?;

    Ok(Self { store, instance })
  }

  fn handle_call(&mut self, msgs: &[ActorMsg], state: &ActorState) -> Result<ActorState> {
    self
      .instance
      .wasm_actor()
      .call_handle_call(&mut self.store, msgs, state)
      .map_err(|err| anyhow!("call_handle_call failed: {err}"))
  }

  fn migrate_state(&mut self, state: &ActorState) -> Result<ActorState> {
    self
      .instance
      .wasm_actor()
      .call_migrate_state(&mut self.store, state)
      .map_err(|err| anyhow!("migrate_state failed: {err}"))
  }
}

#[test]
fn wasm_does_not_call_host_before_push_interval() -> Result<()> {
  let mut host = MockHostGateway::new();
  host.expect_send_to_host().times(0);

  let mut actor = MockedActor::new("guest-v1", host)?;
  let initial = initial_actor_state(0, 7, 10, 0);
  let actual = actor.handle_call(&tick_messages(5), &initial)?;
  let ActorState::V1(actual) = actual else {
    panic!("guest-v1 returned non-v1 state");
  };
  let ActorState::V1(initial) = initial else {
    panic!("initial state should be v1");
  };

  assert_eq!(actual.tick, 5);
  assert_eq!(actual.elapsed_since_push, 2_500);
  assert_eq!(actual.last_host_reply, initial.last_host_reply);
  assert_eq!(actual.last_response.handled, initial.last_response.handled);
  assert_eq!(actual.last_response.reply, initial.last_response.reply);
  assert_eq!(actual.last_response.message, initial.last_response.message);

  Ok(())
}

#[test]
fn wasm_calls_mocked_host_when_push_interval_is_reached() -> Result<()> {
  let mut host = MockHostGateway::new();
  host
    .expect_send_to_host()
    .times(1)
    .withf(|msg| {
      msg.tick == 6
        && msg.last_handled == 41
        && msg.last_host_reply == 13
        && msg.payload == "wasm主动消息 at tick 6"
    })
    .returning(|msg| ActorResponse {
      handled: 99,
      reply: 123,
      message: format!("mocked host reply for {}", msg.payload),
    });

  let mut actor = MockedActor::new("guest-v1", host)?;
  let initial = initial_actor_state(0, 13, 41, 0);
  let actual = actor.handle_call(&tick_messages(6), &initial)?;
  let ActorState::V1(actual) = actual else {
    panic!("guest-v1 returned non-v1 state");
  };

  assert_eq!(actual.tick, 6);
  assert_eq!(actual.elapsed_since_push, 0);
  assert_eq!(actual.last_host_reply, 123);
  assert_eq!(actual.last_response.handled, 99);
  assert_eq!(actual.last_response.reply, 123);
  assert_eq!(
    actual.last_response.message,
    "mocked host reply for wasm主动消息 at tick 6"
  );

  Ok(())
}

#[test]
fn guest_v2_migrates_v1_state_and_handles_calls_with_v2_state() -> Result<()> {
  let mut host = MockHostGateway::new();
  host.expect_send_to_host().times(0);

  let mut actor = MockedActor::new("guest-v2", host)?;
  let initial = initial_actor_state(6, 123, 99, 0);
  let migrated = actor.migrate_state(&initial)?;
  let ActorState::V2(migrated) = migrated else {
    panic!("guest-v2 migrate-state should return v2 state");
  };

  assert_eq!(migrated.tick, 6);
  assert_eq!(migrated.last_host_reply, 123);
  assert_eq!(migrated.last_response.handled, 99);
  assert_eq!(migrated.upgrade_generation, 1);
  assert_eq!(migrated.migrated_from_tick, 6);
  assert_eq!(migrated.host_messages_seen, 0);

  let actual = actor.handle_call(
    &[
      ActorMsg {
        kind: ActorMsgKind::HostMessage,
        host_message: Some(host_actor::HostMessage {
          sequence: 77,
          payload: "queued after upgrade".to_owned(),
        }),
      },
      ActorMsg {
        kind: ActorMsgKind::Tick,
        host_message: None,
      },
    ],
    &ActorState::V2(migrated),
  )?;
  let ActorState::V2(actual) = actual else {
    panic!("guest-v2 handle-call should keep v2 state");
  };

  assert_eq!(actual.tick, 7);
  assert_eq!(actual.upgrade_generation, 1);
  assert_eq!(actual.migrated_from_tick, 6);
  assert_eq!(actual.host_messages_seen, 1);
  assert_eq!(actual.last_host_sequence, 77);
  assert_eq!(actual.last_host_payload, "queued after upgrade");
  assert_eq!(actual.proactive_sends, 0);

  Ok(())
}

fn tick_messages(count: usize) -> Vec<ActorMsg> {
  (0 .. count)
    .map(|_| ActorMsg {
      kind: ActorMsgKind::Tick,
      host_message: None,
    })
    .collect()
}

fn initial_actor_state(
  tick: i32,
  last_host_reply: i32,
  handled: u64,
  elapsed_since_push: i32,
) -> ActorState {
  ActorState::V1(ActorStateV1 {
    tick,
    last_host_reply,
    elapsed_since_push,
    last_response: ActorResponse {
      handled,
      reply: last_host_reply,
      message: format!("initial handled {handled}"),
    },
  })
}

fn ensure_guest_component(feature: &'static str) -> Result<PathBuf> {
  let package_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
  let workspace_dir = package_dir
    .parent()
    .context("failed to determine workspace root for wasmtime_actor")?;
  let guest_target_dir = workspace_dir
    .join("target")
    .join("wasmtime-actor-mockall-guest")
    .join(feature);
  let guest_component = guest_target_dir
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
    .arg(&guest_target_dir)
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
         `rustup target add {GUEST_TARGET}` and rerun `cargo test -p wasmtime_actor --test \
         wasm_actor_mockall`.\n\ncargo stderr:\n{stderr}"
      );
    }

    bail!(
      "failed to build {feature} guest component before running mockall tests.\nmanifest: \
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
