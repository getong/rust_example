use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use tokio::{
  sync::{mpsc, oneshot},
  task::JoinHandle,
};
use wasmtime::Engine;

use crate::{
  types::{Request, Response, ServiceSnapshot, State},
  wasm_rule::WasmHandler,
};

// ---------------------------------------------------------------------------
// Internal actor messages
// ---------------------------------------------------------------------------

enum WasmCommand {
  Call {
    request: Request,
    reply: oneshot::Sender<Result<Response>>,
  },
  Upgrade {
    wasm_path: PathBuf,
    reply: oneshot::Sender<Result<String>>,
  },
  Snapshot {
    reply: oneshot::Sender<ServiceSnapshot>,
  },
  Stop {
    reply: oneshot::Sender<()>,
  },
}

// ---------------------------------------------------------------------------
// Actor internals
// ---------------------------------------------------------------------------

struct WasmHotService {
  /// Shared engine – cheaply clonable (Arc-based internally).
  engine: Engine,
  state: State,
  handler: WasmHandler,
  rx: mpsc::Receiver<WasmCommand>,
}

impl WasmHotService {
  async fn run(mut self) {
    while let Some(command) = self.rx.recv().await {
      match command {
        WasmCommand::Call { request, reply } => {
          let response = self.handler.handle(&mut self.state, request);
          let _ = reply.send(response);
        }
        WasmCommand::Upgrade { wasm_path, reply } => {
          let result = self.upgrade(wasm_path);
          let _ = reply.send(result);
        }
        WasmCommand::Snapshot { reply } => {
          let _ = reply.send(self.snapshot());
        }
        WasmCommand::Stop { reply } => {
          let _ = reply.send(());
          break;
        }
      }
    }
  }

  fn upgrade(&mut self, wasm_path: PathBuf) -> Result<String> {
    let old_version = self.handler.version().to_owned();
    let mut next = WasmHandler::load(&self.engine, &wasm_path)?;

    // Dry-run against a shadow copy of state to validate before committing.
    self.validate(&mut next)?;

    next.migrate_state(&mut self.state)?;
    self.state.upgrades += 1;
    self.handler = next;

    Ok(format!(
      "upgrade {old_version} -> {}",
      self.handler.version()
    ))
  }

  fn validate(&self, next: &mut WasmHandler) -> Result<()> {
    let mut shadow = self.state.clone();
    next.migrate_state(&mut shadow)?;

    let response = next.handle(
      &mut shadow,
      Request {
        user_id: 2,
        amount: 100,
      },
    )?;

    if response.rule_version.trim().is_empty() {
      return Err(anyhow!("new wasm rule returned an empty version"));
    }

    Ok(())
  }

  fn snapshot(&self) -> ServiceSnapshot {
    ServiceSnapshot {
      processed: self.state.processed,
      schema_version: self.state.schema_version,
      fast_lane_hits: self.state.fast_lane_hits,
      upgrades: self.state.upgrades,
      current_rule_version: self.handler.version().to_owned(),
    }
  }
}

// ---------------------------------------------------------------------------
// Public handle – callers never touch actor internals directly
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct WasmServiceHandle {
  tx: mpsc::Sender<WasmCommand>,
}

impl WasmServiceHandle {
  pub async fn call(&self, request: Request) -> Result<Response> {
    let (reply, rx) = oneshot::channel();
    self
      .tx
      .send(WasmCommand::Call { request, reply })
      .await
      .context("wasm service is not available")?;
    rx.await.context("wasm service dropped call reply")?
  }

  pub async fn upgrade(&self, wasm_path: impl Into<PathBuf>) -> Result<String> {
    let (reply, rx) = oneshot::channel();
    self
      .tx
      .send(WasmCommand::Upgrade {
        wasm_path: wasm_path.into(),
        reply,
      })
      .await
      .context("wasm service is not available")?;
    rx.await.context("wasm service dropped upgrade reply")?
  }

  pub async fn snapshot(&self) -> Result<ServiceSnapshot> {
    let (reply, rx) = oneshot::channel();
    self
      .tx
      .send(WasmCommand::Snapshot { reply })
      .await
      .context("wasm service is not available")?;
    rx.await.context("wasm service dropped snapshot reply")
  }

  pub async fn stop(&self) -> Result<()> {
    let (reply, rx) = oneshot::channel();
    self
      .tx
      .send(WasmCommand::Stop { reply })
      .await
      .context("wasm service is not available")?;
    rx.await.context("wasm service dropped stop reply")?;
    Ok(())
  }
}

// ---------------------------------------------------------------------------
// Started service wrapper (owns the background task)
// ---------------------------------------------------------------------------

pub struct WasmStartedService {
  handle: WasmServiceHandle,
  task: JoinHandle<()>,
}

impl WasmStartedService {
  pub fn handle(&self) -> WasmServiceHandle {
    self.handle.clone()
  }

  pub async fn shutdown(self) -> Result<()> {
    self.handle.stop().await?;
    self.task.await.context("wasm service task panicked")?;
    Ok(())
  }
}

// ---------------------------------------------------------------------------
// Constructor
// ---------------------------------------------------------------------------

/// Start the WASM-based hot-upgrade service, loading the initial rule from
/// `initial_rule` (a `.wasm` file path).
pub fn start_wasm_service(initial_rule: impl AsRef<Path>) -> Result<WasmStartedService> {
  let engine = Engine::default();
  let initial_rule = initial_rule.as_ref();
  let initial = WasmHandler::load(&engine, initial_rule).with_context(|| {
    format!(
      "failed to load initial wasm rule {}",
      initial_rule.display()
    )
  })?;

  let mut state = State::default();
  initial.migrate_state(&mut state)?;

  let (tx, rx) = mpsc::channel(64);
  let task = tokio::spawn(
    WasmHotService {
      engine,
      state,
      handler: initial,
      rx,
    }
    .run(),
  );

  Ok(WasmStartedService {
    handle: WasmServiceHandle { tx },
    task,
  })
}
