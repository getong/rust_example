use std::{collections::HashMap, ops::ControlFlow, time::Duration};

use kameo::{error::Infallible, prelude::*};
use tokio::task::AbortHandle;

use crate::player::{PlayerActor, PlayerId};

pub(crate) struct TcpConnectionMonitor {
  retention: Duration,
  players: HashMap<PlayerId, MonitoredPlayer>,
}

impl Actor for TcpConnectionMonitor {
  type Args = Self;
  type Error = Infallible;

  async fn on_start(args: Self::Args, _actor_ref: ActorRef<Self>) -> Result<Self, Self::Error> {
    Ok(args)
  }

  async fn on_link_died(
    &mut self,
    _actor_ref: WeakActorRef<Self>,
    id: ActorId,
    reason: ActorStopReason,
  ) -> Result<ControlFlow<ActorStopReason>, Self::Error> {
    println!("[tcp_monitor] linked actor {id} stopped: {reason:?}");
    Ok(ControlFlow::Continue(()))
  }
}

impl TcpConnectionMonitor {
  pub(crate) fn new(retention: Duration) -> Self {
    Self {
      retention,
      players: HashMap::new(),
    }
  }
}

struct MonitoredPlayer {
  player: ActorRef<PlayerActor>,
  state: ConnectionState,
}

enum ConnectionState {
  Connected {
    connection_id: String,
  },
  Retained {
    last_connection_id: String,
    abort_handle: AbortHandle,
  },
}

pub(crate) struct BindTcpConnection {
  pub(crate) player_id: PlayerId,
  pub(crate) connection_id: String,
  pub(crate) player: ActorRef<PlayerActor>,
}

#[derive(Debug, Reply)]
pub(crate) struct BindReport {
  pub(crate) player_id: PlayerId,
  pub(crate) connection_id: String,
  pub(crate) reconnected: bool,
}

impl BindReport {
  pub(crate) fn log(&self) {
    println!(
      "[player_node] tcp bind: player:{} connection={} reconnected={}",
      self.player_id, self.connection_id, self.reconnected
    );
  }
}

impl Message<BindTcpConnection> for TcpConnectionMonitor {
  type Reply = BindReport;

  async fn handle(
    &mut self,
    BindTcpConnection {
      player_id,
      connection_id,
      player,
    }: BindTcpConnection,
    ctx: &mut Context<Self, Self::Reply>,
  ) -> Self::Reply {
    let reconnected = self
      .players
      .get(&player_id)
      .is_some_and(|entry| matches!(entry.state, ConnectionState::Retained { .. }));

    if let Some(entry) = self.players.get_mut(&player_id) {
      if let ConnectionState::Retained { abort_handle, .. } = &entry.state {
        abort_handle.abort();
      }
      entry.player = player.clone();
      entry.state = ConnectionState::Connected {
        connection_id: connection_id.clone(),
      };
    } else {
      self.players.insert(
        player_id,
        MonitoredPlayer {
          player: player.clone(),
          state: ConnectionState::Connected {
            connection_id: connection_id.clone(),
          },
        },
      );
    }

    ctx.actor_ref().link(&player).await;

    if reconnected {
      println!(
        "[tcp_monitor] player:{player_id} reconnected on {connection_id}; retention timer \
         cancelled"
      );
    } else {
      println!("[tcp_monitor] player:{player_id} bound to tcp connection {connection_id}");
    }

    BindReport {
      player_id,
      connection_id,
      reconnected,
    }
  }
}

pub(crate) struct TcpDisconnected {
  pub(crate) player_id: PlayerId,
  pub(crate) connection_id: String,
}

impl Message<TcpDisconnected> for TcpConnectionMonitor {
  type Reply = bool;

  async fn handle(
    &mut self,
    TcpDisconnected {
      player_id,
      connection_id,
    }: TcpDisconnected,
    ctx: &mut Context<Self, Self::Reply>,
  ) -> Self::Reply {
    let Some(player) = self.players.get_mut(&player_id) else {
      return false;
    };

    let ConnectionState::Connected {
      connection_id: current_connection,
    } = &player.state
    else {
      return false;
    };

    if current_connection != &connection_id {
      return false;
    }

    let monitor = ctx.actor_ref().downgrade();
    let retention = self.retention;
    let timer_connection = connection_id.clone();
    let abort_handle = tokio::spawn(async move {
      tokio::time::sleep(retention).await;
      if let Some(monitor) = monitor.upgrade() {
        let _ = monitor
          .tell(RetentionExpired {
            player_id,
            connection_id: timer_connection,
          })
          .await;
      }
    })
    .abort_handle();

    player.state = ConnectionState::Retained {
      last_connection_id: connection_id.clone(),
      abort_handle,
    };

    println!(
      "[tcp_monitor] player:{player_id} tcp connection {connection_id} disconnected; retaining \
       player actor for {} s",
      self.retention.as_secs()
    );
    true
  }
}

struct RetentionExpired {
  player_id: PlayerId,
  connection_id: String,
}

impl Message<RetentionExpired> for TcpConnectionMonitor {
  type Reply = ();

  async fn handle(
    &mut self,
    RetentionExpired {
      player_id,
      connection_id,
    }: RetentionExpired,
    _ctx: &mut Context<Self, Self::Reply>,
  ) -> Self::Reply {
    let should_disconnect = self.players.get(&player_id).is_some_and(|entry| {
      matches!(
        &entry.state,
        ConnectionState::Retained { last_connection_id, .. } if last_connection_id == &connection_id
      )
    });

    if !should_disconnect {
      return;
    }

    let Some(player) = self.players.remove(&player_id) else {
      return;
    };

    println!(
      "[tcp_monitor] player:{player_id} had no tcp reconnect within retention window; stopping \
       player actor"
    );

    if let Err(err) = player.player.stop_gracefully().await {
      println!("[tcp_monitor] failed to stop player:{player_id}: {err}");
    }
    player.player.wait_for_shutdown().await;
  }
}

pub(crate) struct GetTcpConnections;

#[derive(Clone, Debug)]
pub(crate) struct TcpConnectionSnapshot {
  pub(crate) player_id: PlayerId,
  pub(crate) state: TcpConnectionStateSnapshot,
}

#[derive(Clone, Debug)]
pub(crate) enum TcpConnectionStateSnapshot {
  Connected { connection_id: String },
  Retained { last_connection_id: String },
}

impl TcpConnectionSnapshot {
  pub(crate) fn log(&self) {
    match &self.state {
      TcpConnectionStateSnapshot::Connected { connection_id } => println!(
        "[player_node] tcp monitor: player:{} connected via {}",
        self.player_id, connection_id
      ),
      TcpConnectionStateSnapshot::Retained { last_connection_id } => println!(
        "[player_node] tcp monitor: player:{} retained after {}",
        self.player_id, last_connection_id
      ),
    }
  }
}

impl Message<GetTcpConnections> for TcpConnectionMonitor {
  type Reply = Vec<TcpConnectionSnapshot>;

  async fn handle(
    &mut self,
    _msg: GetTcpConnections,
    _ctx: &mut Context<Self, Self::Reply>,
  ) -> Self::Reply {
    self
      .players
      .iter()
      .map(|(&player_id, player)| TcpConnectionSnapshot {
        player_id,
        state: match &player.state {
          ConnectionState::Connected { connection_id } => TcpConnectionStateSnapshot::Connected {
            connection_id: connection_id.clone(),
          },
          ConnectionState::Retained {
            last_connection_id, ..
          } => TcpConnectionStateSnapshot::Retained {
            last_connection_id: last_connection_id.clone(),
          },
        },
      })
      .collect()
  }
}
