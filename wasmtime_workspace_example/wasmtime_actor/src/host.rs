use std::{
  env,
  path::{Path, PathBuf},
  process::Command,
  sync::mpsc,
  thread,
  time::Duration,
};

use anyhow::{Context, Result, anyhow, bail};
use wasmtime::{
  Config, Engine, Store,
  component::{Component, HasSelf, Linker},
};
use wasmtime_wasi::{ResourceTable, WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};

use crate::bindings::{ActorWorld, demo::actor::host_actor};

const GUEST_TARGET: &str = "wasm32-wasip2";
const GUEST_WASM: &str = "wasmtime_actor.wasm";

/// A command sent from the WASM guest (via StoreState) to the host actor thread.
///
/// Carries a fully-typed `GuestMessage` (generated from actor.wit by both
/// wit_bindgen and wasmtime bindgen) — no JSON serialization on the hot path.
enum HostCommand {
  Message {
    msg: host_actor::GuestMessage,
    /// oneshot reply channel: host sends the typed ActorResponse back here.
    reply_tx: mpsc::SyncSender<host_actor::ActorResponse>,
  },
}

/// All state shared between the wasmtime Store and the WIT host-actor implementation.
///
/// wasmtime calls `host_actor::Host for StoreState` on every WIT import invoked by the
/// WASM guest.  The guest cannot name this type — it sees only the typed WIT functions —
/// but every call goes through this struct.
///
/// Channel layout (fully typed, zero JSON):
///   host actor thread  ←──(HostCommand)──────────────────  StoreState.host_actor_tx
///   host actor thread  ──(host_actor::HostMessage)──────→  StoreState.host_to_wasm_rx
pub struct StoreState {
  /// WASM sends typed GuestMessage requests through this sender.
  host_actor_tx: mpsc::Sender<HostCommand>,
  /// WASM polls typed HostMessage values pushed by the host actor thread.
  host_to_wasm_rx: mpsc::Receiver<host_actor::HostMessage>,
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

/// The `host_actor::Host` impl is the **only** path the WASM guest uses to reach
/// `StoreState`.  wasmtime routes every WIT import call here automatically.
///
/// Method signatures are fully typed (generated from actor.wit) — the guest sends
/// a `GuestMessage` struct and receives an `ActorResponse` struct, both crossing
/// the WASM boundary via the component-model ABI with no JSON involved.
impl host_actor::Host for StoreState {
  /// Synchronous request-reply: moves the typed GuestMessage to the host thread
  /// via StoreState.host_actor_tx and blocks until the host replies with a typed
  /// ActorResponse via the per-call sync_channel(1) oneshot.
  fn send_to_host(&mut self, msg: host_actor::GuestMessage) -> host_actor::ActorResponse {
    let (reply_tx, reply_rx) = mpsc::sync_channel(1);
    self
      .host_actor_tx
      .send(HostCommand::Message { msg, reply_tx })
      .expect("host actor thread disconnected");
    reply_rx
      .recv()
      .expect("host actor thread closed reply channel")
  }

  /// Non-blocking poll: returns the next typed HostMessage from
  /// StoreState.host_to_wasm_rx, or None if the queue is empty.
  fn recv_from_host(&mut self) -> Option<host_actor::HostMessage> {
    self.host_to_wasm_rx.try_recv().ok()
  }

  fn sleep_millis(&mut self, millis: i32) {
    if millis > 0 {
      let bounded = millis.min(60_000) as u64;
      thread::sleep(Duration::from_millis(bounded));
    }
  }
}

/// Spawns the host actor thread.
///
/// The thread:
/// 1. Pushes the initial 3 proactive messages into `msg_tx` (WASM receives them via
///    `recv_from_host`).
/// 2. Processes every `HostCommand::Message` sent by the WASM guest, replies synchronously via the
///    per-command oneshot channel.
/// 3. Exits when `cmd_rx` is closed (i.e. `StoreState` is dropped).
///
/// Returns the number of guest messages handled so it can be printed after the WASM
/// loop finishes.
fn spawn_host_actor(
  cmd_rx: mpsc::Receiver<HostCommand>,
  msg_tx: mpsc::Sender<host_actor::HostMessage>,
) -> thread::JoinHandle<u64> {
  thread::spawn(move || {
    // Push initial proactive messages as typed HostMessage structs — no JSON.
    for sequence in 1u64 ..= 3 {
      let msg = host_actor::HostMessage {
        sequence,
        payload: format!("host queued message {sequence}"),
      };
      println!(
        "host: queued message for wasm: seq={} payload={}",
        msg.sequence, msg.payload
      );
      msg_tx.send(msg).ok();
    }

    let mut host_handled = 0u64;
    while let Ok(cmd) = cmd_rx.recv() {
      match cmd {
        // msg is a typed GuestMessage — fields are directly accessible, no JSON decode.
        HostCommand::Message { msg, reply_tx } => {
          host_handled += 1;
          let reply =
            ((msg.tick as i64 + msg.last_host_reply as i64 + host_handled as i64) % 997) as i32;
          let response = host_actor::ActorResponse {
            handled: host_handled,
            reply,
            message: format!("host processed wasm主动消息 `{}`", msg.payload),
          };
          println!(
            "host actor: tick={} payload={}, handled #{host_handled}, reply={reply}",
            msg.tick, msg.payload
          );
          reply_tx.send(response).ok();
        }
      }
    }
    host_handled
  })
}

pub fn run() -> Result<()> {
  let max_ticks = max_ticks_from_env()?;
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

  let (host_actor_tx, host_cmd_rx) = mpsc::channel::<HostCommand>();
  let (host_msg_tx, host_to_wasm_rx) = mpsc::channel::<host_actor::HostMessage>();
  let host_handle = spawn_host_actor(host_cmd_rx, host_msg_tx);

  let mut store = Store::new(
    &engine,
    StoreState {
      host_actor_tx,
      host_to_wasm_rx,
      wasi: WasiCtxBuilder::new().inherit_stdio().build(),
      table: ResourceTable::new(),
    },
  );
  let instance = ActorWorld::instantiate(&mut store, &component, &linker)
    .map_err(|err| anyhow!("failed to instantiate guest actor component: {err}"))?;

  if max_ticks == 0 {
    println!(
      "host: native actor loop and in-process wasm guest loop are running forever; press Ctrl+C \
       to stop"
    );
  } else {
    println!("host: running in-process resident loops for {max_ticks} ticks for verification");
  }

  let result = instance
    .wasm_actor()
    .call_run_loop(&mut store, max_ticks)
    .map_err(|err| anyhow!("guest loop failed: {err}"))?;
  println!("host: wasm guest loop returned: {result}");

  // Drop the store so host_actor_tx is closed, which lets the host thread exit.
  drop(store);
  let host_handled = host_handle.join().unwrap_or(0);
  println!("host: host actor thread handled {host_handled} wasm主动 messages");

  Ok(())
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

fn ensure_guest_component() -> Result<PathBuf> {
  let package_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
  let workspace_dir = package_dir
    .parent()
    .context("failed to determine workspace root for wasmtime_actor")?;
  let guest_target_dir = workspace_dir.join("target").join("wasmtime-actor-guest");
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
         `rustup target add {GUEST_TARGET}` and rerun `cargo run -p wasmtime_actor --bin \
         wasmtime_actor`.\n\ncargo stderr:\n{stderr}"
      );
    }

    bail!(
      "failed to build the guest component before launching the host actor.\nmanifest: \
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
