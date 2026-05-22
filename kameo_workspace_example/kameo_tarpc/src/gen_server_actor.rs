use std::time::Duration;

use anyhow::anyhow;
use kameo::{mailbox::Signal, prelude::*};
use tokio::{
  sync::{mpsc, mpsc::error::TryRecvError},
  time::{Instant, Interval, MissedTickBehavior, interval_at, sleep},
};
use tracing::{info, warn};

pub struct GenServerActor {
  name: String,
  value: i64,
  heartbeat_count: u64,
  history: Vec<String>,
  external_rx: mpsc::Receiver<ExternalEvent>,
  external_rx_closed: bool,
  prefer_external: bool,
  heartbeat: Interval,
}

#[derive(Debug, Clone)]
pub enum ExternalEvent {
  Push(String),
  Multiply(i64),
  Reset,
}

#[derive(Debug, Clone, Reply)]
pub struct GenServerState {
  pub name: String,
  pub value: i64,
  pub heartbeat_count: u64,
  pub history: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct CastAdd {
  pub amount: i64,
  pub reason: String,
}

#[derive(Debug, Clone)]
pub struct CallAdd {
  pub amount: i64,
  pub reason: String,
}

#[derive(Debug, Clone)]
pub struct GetState;

#[derive(Debug, Clone)]
pub struct StopServer;

impl GenServerActor {
  pub fn new(
    name: impl Into<String>,
    heartbeat_every: Duration,
  ) -> (Self, mpsc::Sender<ExternalEvent>) {
    let (external_tx, external_rx) = mpsc::channel(32);
    let mut heartbeat = interval_at(Instant::now() + heartbeat_every, heartbeat_every);
    heartbeat.set_missed_tick_behavior(MissedTickBehavior::Delay);

    (
      Self {
        name: name.into(),
        value: 0,
        heartbeat_count: 0,
        history: Vec::new(),
        external_rx,
        external_rx_closed: false,
        prefer_external: false,
        heartbeat,
      },
      external_tx,
    )
  }

  fn snapshot(&self) -> GenServerState {
    GenServerState {
      name: self.name.clone(),
      value: self.value,
      heartbeat_count: self.heartbeat_count,
      history: self.history.clone(),
    }
  }

  fn push_history(&mut self, entry: impl Into<String>) {
    self.history.push(entry.into());
    if self.history.len() > 12 {
      self.history.remove(0);
    }
  }

  fn handle_external_event(&mut self, event: ExternalEvent) {
    match event {
      ExternalEvent::Push(text) => {
        self.push_history(format!("external push {text}"));
        info!(name = %self.name, %text, "handled external push");
      }
      ExternalEvent::Multiply(factor) => {
        self.value *= factor;
        self.push_history(format!("external multiply by {factor}"));
        info!(
          name = %self.name,
          factor,
          value = self.value,
          "handled external multiply"
        );
      }
      ExternalEvent::Reset => {
        self.value = 0;
        self.push_history("external reset");
        info!(name = %self.name, "handled external reset");
      }
    }
  }

  fn handle_heartbeat_tick(&mut self) {
    self.heartbeat_count += 1;
    self.push_history(format!("heartbeat {}", self.heartbeat_count));
    info!(
      name = %self.name,
      heartbeat_count = self.heartbeat_count,
      value = self.value,
      "handled heartbeat tick"
    );
  }
}

impl Actor for GenServerActor {
  type Args = Self;
  type Error = anyhow::Error;

  async fn on_start(state: Self::Args, actor_ref: ActorRef<Self>) -> Result<Self, Self::Error> {
    info!(actor_id = %actor_ref.id(), name = %state.name, "gen_server actor started");
    Ok(state)
  }

  async fn next(
    &mut self,
    _actor_ref: WeakActorRef<Self>,
    mailbox_rx: &mut MailboxReceiver<Self>,
  ) -> Result<Option<Signal<Self>>, Self::Error> {
    loop {
      if self.prefer_external {
        if self.try_handle_external_event() {
          self.prefer_external = false;
          continue;
        }

        match mailbox_rx.try_recv() {
          Ok(signal) => {
            self.prefer_external = true;
            return Ok(Some(signal));
          }
          Err(TryRecvError::Empty) => {}
          Err(TryRecvError::Disconnected) => return Ok(None),
        }
      } else {
        match mailbox_rx.try_recv() {
          Ok(signal) => {
            self.prefer_external = true;
            return Ok(Some(signal));
          }
          Err(TryRecvError::Empty) => {}
          Err(TryRecvError::Disconnected) => return Ok(None),
        }

        if self.try_handle_external_event() {
          self.prefer_external = false;
          continue;
        }
      }

      tokio::select! {
        signal = mailbox_rx.recv() => {
          self.prefer_external = true;
          return Ok(signal);
        }
        event = self.external_rx.recv(), if !self.external_rx_closed => {
          match event {
            Some(event) => {
              self.prefer_external = false;
              self.handle_external_event(event);
              continue;
            }
            None => {
              self.external_rx_closed = true;
              warn!(name = %self.name, "external event channel closed");
            }
          }
        }
        _ = self.heartbeat.tick() => {
          self.handle_heartbeat_tick();
          continue;
        }
      }
    }
  }

  async fn on_stop(
    &mut self,
    _actor_ref: WeakActorRef<Self>,
    reason: ActorStopReason,
  ) -> Result<(), Self::Error> {
    info!(name = %self.name, ?reason, "gen_server actor stopped");
    Ok(())
  }
}

impl GenServerActor {
  fn try_handle_external_event(&mut self) -> bool {
    if self.external_rx_closed {
      return false;
    }

    match self.external_rx.try_recv() {
      Ok(event) => {
        self.handle_external_event(event);
        true
      }
      Err(TryRecvError::Empty) => false,
      Err(TryRecvError::Disconnected) => {
        self.external_rx_closed = true;
        warn!(name = %self.name, "external event channel closed");
        false
      }
    }
  }
}

impl Message<CastAdd> for GenServerActor {
  type Reply = ();

  async fn handle(&mut self, msg: CastAdd, _ctx: &mut Context<Self, Self::Reply>) -> Self::Reply {
    self.value += msg.amount;
    self.push_history(format!("cast add {} ({})", msg.amount, msg.reason));
    info!(
      name = %self.name,
      amount = msg.amount,
      value = self.value,
      "handled cast add"
    );
  }
}

impl Message<CallAdd> for GenServerActor {
  type Reply = GenServerState;

  async fn handle(&mut self, msg: CallAdd, _ctx: &mut Context<Self, Self::Reply>) -> Self::Reply {
    self.value += msg.amount;
    self.push_history(format!("call add {} ({})", msg.amount, msg.reason));
    info!(
      name = %self.name,
      amount = msg.amount,
      value = self.value,
      "handled call add"
    );
    self.snapshot()
  }
}

impl Message<GetState> for GenServerActor {
  type Reply = GenServerState;

  async fn handle(&mut self, _msg: GetState, _ctx: &mut Context<Self, Self::Reply>) -> Self::Reply {
    self.snapshot()
  }
}

impl Message<StopServer> for GenServerActor {
  type Reply = GenServerState;

  async fn handle(
    &mut self,
    _msg: StopServer,
    ctx: &mut Context<Self, Self::Reply>,
  ) -> Self::Reply {
    self.push_history("stop requested");
    ctx.stop();
    self.snapshot()
  }
}

pub async fn run_gen_server_demo() -> anyhow::Result<()> {
  let (server, external_tx) = GenServerActor::new("gen-server-demo", Duration::from_millis(200));
  let server = GenServerActor::spawn(server);
  server.wait_for_startup().await;

  server
    .tell(CastAdd {
      amount: 5,
      reason: "fire-and-forget cast".to_string(),
    })
    .await
    .map_err(|err| anyhow!("cast add failed: {err:?}"))?;

  let after_call = server
    .ask(CallAdd {
      amount: 7,
      reason: "synchronous call".to_string(),
    })
    .await
    .map_err(|err| anyhow!("call add failed: {err:?}"))?;
  print_state("after call", &after_call);

  external_tx
    .send(ExternalEvent::Push("from external channel".to_string()))
    .await
    .map_err(|err| anyhow!("send external push failed: {err}"))?;
  external_tx
    .send(ExternalEvent::Multiply(3))
    .await
    .map_err(|err| anyhow!("send external multiply failed: {err}"))?;

  sleep(Duration::from_millis(650)).await;

  let after_select = server
    .ask(GetState)
    .await
    .map_err(|err| anyhow!("get state failed: {err:?}"))?;
  print_state("after external events and ticks", &after_select);

  external_tx
    .send(ExternalEvent::Reset)
    .await
    .map_err(|err| anyhow!("send external reset failed: {err}"))?;
  sleep(Duration::from_millis(50)).await;

  let stopped = server
    .ask(StopServer)
    .await
    .map_err(|err| anyhow!("stop server failed: {err:?}"))?;
  print_state("stopping", &stopped);
  server.wait_for_shutdown().await;

  Ok(())
}

fn print_state(label: &str, state: &GenServerState) {
  println!(
    "{label}: name={} value={} heartbeat_count={} history={:?}",
    state.name, state.value, state.heartbeat_count, state.history
  );
}

#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  async fn select_loop_handles_mailbox_external_events_and_ticks() -> anyhow::Result<()> {
    let (server, external_tx) = GenServerActor::new("test-gen-server", Duration::from_millis(10));
    let server = GenServerActor::spawn(server);
    server.wait_for_startup().await;

    server
      .tell(CastAdd {
        amount: 2,
        reason: "test cast".to_string(),
      })
      .await
      .map_err(|err| anyhow!("cast add failed: {err:?}"))?;

    let after_cast = server
      .ask(GetState)
      .await
      .map_err(|err| anyhow!("get state after cast failed: {err:?}"))?;
    assert_eq!(after_cast.value, 2);

    external_tx
      .send(ExternalEvent::Multiply(4))
      .await
      .map_err(|err| anyhow!("send external multiply failed: {err}"))?;

    let mut state = after_cast;

    for _ in 0 .. 10 {
      if state.value == 8 && state.heartbeat_count > 0 {
        break;
      }
      sleep(Duration::from_millis(10)).await;
      state = server
        .ask(GetState)
        .await
        .map_err(|err| anyhow!("get state failed: {err:?}"))?;
    }

    assert_eq!(state.value, 8);
    assert!(state.heartbeat_count > 0);
    assert!(
      state
        .history
        .iter()
        .any(|entry| entry == "external multiply by 4")
    );

    server
      .ask(StopServer)
      .await
      .map_err(|err| anyhow!("stop server failed: {err:?}"))?;
    server.wait_for_shutdown().await;

    Ok(())
  }
}
