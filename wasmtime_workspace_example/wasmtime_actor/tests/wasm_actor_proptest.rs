use std::{
  env,
  fmt::Write as _,
  path::{Path, PathBuf},
  process::Command,
};

use anyhow::{Context, Result, anyhow, bail};
use proptest::prelude::*;
use serde_json::Value;
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
const LOOP_SLEEP_MILLIS: i32 = 500;
const WASM_TO_HOST_INTERVAL_MILLIS: i32 = 3_000;

struct TestStoreState {
  host_callbacks: u64,
  wasi: WasiCtx,
  table: ResourceTable,
}

impl WasiView for TestStoreState {
  fn ctx(&mut self) -> WasiCtxView<'_> {
    WasiCtxView {
      ctx: &mut self.wasi,
      table: &mut self.table,
    }
  }
}

impl host_actor::Host for TestStoreState {
  fn send_to_host(&mut self, msg: host_actor::GuestMessage) -> host_actor::ActorResponse {
    self.host_callbacks += 1;
    let reply =
      ((msg.tick as i64 + msg.last_host_reply as i64 + self.host_callbacks as i64) % 997) as i32;
    let message = {
      let mut buf = String::new();
      write!(
        buf,
        "test host processed `{}` after {} handled messages",
        msg.payload, msg.last_handled,
      )
      .unwrap();
      buf
    };
    ActorResponse {
      handled: self.host_callbacks,
      reply,
      message,
    }
  }
}

struct TestActor {
  store: Store<TestStoreState>,
  instance: ActorWorld,
}

impl TestActor {
  fn new() -> Result<Self> {
    let guest_component = ensure_guest_component("guest-v1")?;

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
      TestStoreState {
        host_callbacks: 0,
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

  fn render_state(&mut self, state: &ActorState) -> Result<String> {
    self
      .instance
      .wasm_actor()
      .call_render_state(&mut self.store, state)
      .map_err(|err| anyhow!("render_state failed: {err}"))
  }
}

#[derive(Clone, Debug)]
enum InputMsg {
  Tick,
  HostMessage { sequence: u64, payload: String },
}

impl InputMsg {
  fn into_actor_msg(self) -> ActorMsg {
    match self {
      Self::Tick => ActorMsg {
        kind: ActorMsgKind::Tick,
        host_message: None,
      },
      Self::HostMessage { sequence, payload } => ActorMsg {
        kind: ActorMsgKind::HostMessage,
        host_message: Some(host_actor::HostMessage { sequence, payload }),
      },
    }
  }
}

fn arb_input_msg() -> impl Strategy<Value = InputMsg> {
  prop_oneof![
    Just(InputMsg::Tick),
    (0u64 .. 1_000_000, "[a-zA-Z0-9 _-]{0,32}")
      .prop_map(|(sequence, payload)| InputMsg::HostMessage { sequence, payload }),
  ]
}

proptest! {
  #![proptest_config(ProptestConfig {
    cases: 32,
    max_shrink_iters: 512,
    .. ProptestConfig::default()
  })]

  #[test]
  fn wasm_handle_call_matches_state_model(
    initial_tick in 0i32 .. 10_000,
    initial_last_host_reply in 0i32 .. 997,
    initial_elapsed in 0i32 .. WASM_TO_HOST_INTERVAL_MILLIS,
    initial_handled in 0u64 .. 1_000,
    inputs in prop::collection::vec(arb_input_msg(), 0 .. 80),
  ) {
    let mut actor = TestActor::new().map_err(|err| TestCaseError::fail(err.to_string()))?;
    let initial_state = ActorStateV1 {
      tick: initial_tick,
      last_host_reply: initial_last_host_reply,
      elapsed_since_push: initial_elapsed,
      last_response: ActorResponse {
        handled: initial_handled,
        reply: initial_last_host_reply,
        message: format!("initial response {initial_handled}"),
      },
    };
    let initial_actor_state = ActorState::V1(initial_state.clone());

    let msgs: Vec<_> = inputs
      .clone()
      .into_iter()
      .map(InputMsg::into_actor_msg)
      .collect();
    let expected = model_handle_call(&inputs, &initial_state, 0);
    let actual = actor
      .handle_call(&msgs, &initial_actor_state)
      .map_err(|err| TestCaseError::fail(err.to_string()))?;
    let ActorState::V1(actual) = actual else {
      return Err(TestCaseError::fail("guest-v1 returned non-v1 state"));
    };

    prop_assert_eq!(actual.tick, expected.state.tick);
    prop_assert_eq!(actual.elapsed_since_push, expected.state.elapsed_since_push);
    prop_assert_eq!(actual.last_host_reply, expected.state.last_host_reply);
    prop_assert_eq!(actual.last_response.handled, expected.state.last_response.handled);
    prop_assert_eq!(actual.last_response.reply, expected.state.last_response.reply);
    prop_assert_eq!(actual.last_response.message, expected.state.last_response.message);
    prop_assert_eq!(actor.store.data().host_callbacks, expected.host_callbacks);
  }

  #[test]
  fn wasm_render_state_reports_state_fields(
    tick in 0i32 .. 100_000,
    handled in 0u64 .. 1_000_000,
    reply in 0i32 .. 997,
    message in "[a-zA-Z0-9 _`-]{0,64}",
  ) {
    let mut actor = TestActor::new().map_err(|err| TestCaseError::fail(err.to_string()))?;
    let state = ActorState::V1(ActorStateV1 {
      tick,
      last_host_reply: reply,
      elapsed_since_push: 0,
      last_response: ActorResponse {
        handled,
        reply,
        message: message.clone(),
      },
    });

    let rendered = actor
      .render_state(&state)
      .map_err(|err| TestCaseError::fail(err.to_string()))?;
    let value: Value = serde_json::from_str(&rendered)
      .map_err(|err| TestCaseError::fail(format!("render_state did not return JSON: {err}; output={rendered}")))?;

    prop_assert_eq!(&value["tick"], &Value::from(tick));
    prop_assert_eq!(&value["schema"], &Value::from(1));
    prop_assert_eq!(&value["handled"], &Value::from(handled));
    prop_assert_eq!(&value["reply"], &Value::from(reply));
    prop_assert_eq!(&value["message"], &Value::from(message));
  }
}

struct ModelOutcome {
  state: ActorStateV1,
  host_callbacks: u64,
}

fn model_handle_call(
  inputs: &[InputMsg],
  initial: &ActorStateV1,
  initial_host_callbacks: u64,
) -> ModelOutcome {
  let mut state = initial.clone();
  let mut host_callbacks = initial_host_callbacks;

  for input in inputs {
    match input {
      InputMsg::Tick => {
        state.tick += 1;
        state.elapsed_since_push += LOOP_SLEEP_MILLIS;
        if state.elapsed_since_push >= WASM_TO_HOST_INTERVAL_MILLIS {
          state.elapsed_since_push = 0;
          let payload = format!("wasm主动消息 at tick {}", state.tick);
          let last_handled = state.last_response.handled;
          host_callbacks += 1;
          let reply = ((state.tick as i64 + state.last_host_reply as i64 + host_callbacks as i64)
            % 997) as i32;
          let message =
            format!("test host processed `{payload}` after {last_handled} handled messages");
          state.last_host_reply = reply;
          state.last_response = ActorResponse {
            handled: host_callbacks,
            reply,
            message,
          };
        }
      }
      InputMsg::HostMessage { .. } => {}
    }
  }

  ModelOutcome {
    state,
    host_callbacks,
  }
}

fn ensure_guest_component(feature: &'static str) -> Result<PathBuf> {
  let package_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
  let workspace_dir = package_dir
    .parent()
    .context("failed to determine workspace root for wasmtime_actor")?;
  let guest_target_dir = workspace_dir
    .join("target")
    .join("wasmtime-actor-proptest-guest")
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
         wasm_actor_proptest`.\n\ncargo stderr:\n{stderr}"
      );
    }

    bail!(
      "failed to build {feature} guest component before running proptests.\nmanifest: \
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
