use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use tokio::{
  sync::{mpsc, oneshot},
  task::JoinHandle,
};

use crate::{
  rule::RuleEngine,
  types::{Request, Response, ServiceSnapshot, State},
};

enum Command {
  Call {
    request: Request,
    reply: oneshot::Sender<Result<Response>>,
  },
  Upgrade {
    rule_path: PathBuf,
    reply: oneshot::Sender<Result<String>>,
  },
  Snapshot {
    reply: oneshot::Sender<ServiceSnapshot>,
  },
  Stop {
    reply: oneshot::Sender<()>,
  },
}

struct HotService {
  state: State,
  handler: RuleEngine,
  rx: mpsc::Receiver<Command>,
}

impl HotService {
  async fn run(mut self) {
    while let Some(command) = self.rx.recv().await {
      match command {
        Command::Call { request, reply } => {
          let response = self.handler.handle(&mut self.state, request);
          let _ = reply.send(response);
        }
        Command::Upgrade { rule_path, reply } => {
          let response = self.upgrade(rule_path);
          let _ = reply.send(response);
        }
        Command::Snapshot { reply } => {
          let _ = reply.send(self.snapshot());
        }
        Command::Stop { reply } => {
          let _ = reply.send(());
          break;
        }
      }
    }
  }

  fn upgrade(&mut self, rule_path: PathBuf) -> Result<String> {
    let old_version = self.handler.version().to_owned();
    let next = RuleEngine::load(&rule_path)?;

    self.validate(&next)?;
    next.migrate_state(&mut self.state)?;
    self.state.upgrades += 1;
    self.handler = next;

    Ok(format!(
      "upgrade {old_version} -> {}",
      self.handler.version()
    ))
  }

  fn validate(&self, next: &RuleEngine) -> Result<()> {
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
      return Err(anyhow!("new rule returned an empty version"));
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

#[derive(Clone)]
pub struct HotServiceHandle {
  tx: mpsc::Sender<Command>,
}

impl HotServiceHandle {
  pub async fn call(&self, request: Request) -> Result<Response> {
    let (reply, rx) = oneshot::channel();
    self
      .tx
      .send(Command::Call { request, reply })
      .await
      .context("hot service is not available")?;

    rx.await.context("hot service dropped call reply")?
  }

  pub async fn upgrade(&self, rule_path: impl Into<PathBuf>) -> Result<String> {
    let (reply, rx) = oneshot::channel();
    self
      .tx
      .send(Command::Upgrade {
        rule_path: rule_path.into(),
        reply,
      })
      .await
      .context("hot service is not available")?;

    rx.await.context("hot service dropped upgrade reply")?
  }

  pub async fn snapshot(&self) -> Result<ServiceSnapshot> {
    let (reply, rx) = oneshot::channel();
    self
      .tx
      .send(Command::Snapshot { reply })
      .await
      .context("hot service is not available")?;

    rx.await.context("hot service dropped snapshot reply")
  }

  pub async fn stop(&self) -> Result<()> {
    let (reply, rx) = oneshot::channel();
    self
      .tx
      .send(Command::Stop { reply })
      .await
      .context("hot service is not available")?;

    rx.await.context("hot service dropped stop reply")?;
    Ok(())
  }
}

pub struct StartedService {
  handle: HotServiceHandle,
  task: JoinHandle<()>,
}

impl StartedService {
  pub fn handle(&self) -> HotServiceHandle {
    self.handle.clone()
  }

  pub async fn shutdown(self) -> Result<()> {
    self.handle.stop().await?;
    self.task.await.context("hot service task panicked")?;
    Ok(())
  }
}

pub fn start_service(initial_rule: impl AsRef<Path>) -> Result<StartedService> {
  let initial_rule = initial_rule.as_ref();
  let initial = RuleEngine::load(initial_rule)
    .with_context(|| format!("failed to load initial rule {}", initial_rule.display()))?;

  let mut state = State::default();
  initial.migrate_state(&mut state)?;

  let (tx, rx) = mpsc::channel(64);
  let task = tokio::spawn(
    HotService {
      state,
      handler: initial,
      rx,
    }
    .run(),
  );

  Ok(StartedService {
    handle: HotServiceHandle { tx },
    task,
  })
}
