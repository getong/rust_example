use std::{
  collections::{HashMap, VecDeque},
  time::Duration,
};

use anyhow::{Context, Result, bail};
use futures::StreamExt;
use libp2p::{
  Multiaddr, PeerId, StreamProtocol, Swarm, SwarmBuilder, noise, request_response,
  request_response::ProtocolSupport,
  swarm::{NetworkBehaviour, SwarmEvent},
  tcp, yamux,
};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::{
  model::{DistributedTask, SharedReply, TaskRequest, TaskResponse, WorkerJob},
  store::TaskStore,
};

const TASK_PROTOCOL: &str = "/apalis-libp2p-rocksdb/task/1";
const WORKER_REGISTRATION_TASK_ID: &str = "worker-registration";

pub type RequestBehaviour = request_response::json::Behaviour<TaskRequest, TaskResponse>;

#[derive(NetworkBehaviour)]
#[behaviour(prelude = "libp2p::swarm::derive_prelude")]
pub struct TaskBehaviour {
  request_response: RequestBehaviour,
}

#[derive(Debug)]
pub enum NetworkCommand {
  Submit(DistributedTask),
}

pub struct NetworkNode {
  pub peer_id: PeerId,
  pub command_tx: mpsc::Sender<NetworkCommand>,
}

pub async fn spawn_network(
  role: NetworkRole,
  listen: Multiaddr,
  bootnodes: Vec<PeerAddress>,
  store: TaskStore,
  worker_tx: Option<mpsc::Sender<WorkerJob>>,
) -> Result<NetworkNode> {
  let mut swarm = build_swarm()?;
  let peer_id = *swarm.local_peer_id();
  Swarm::listen_on(&mut swarm, listen).context("starting libp2p listener")?;
  store.append_event("node_started", format!("{role:?}:{peer_id}"))?;

  for bootnode in bootnodes {
    swarm.add_peer_address(bootnode.peer_id, bootnode.address.clone());
    Swarm::dial(&mut swarm, bootnode.address.clone())
      .with_context(|| format!("dialing bootnode {}", bootnode.address))?;
  }

  let (command_tx, command_rx) = mpsc::channel(128);
  let runner = NetworkRunner {
    role,
    swarm,
    command_rx,
    store,
    worker_tx,
    workers: Vec::new(),
    next_worker: 0,
    pending: HashMap::new(),
    backlog: VecDeque::new(),
  };

  tokio::spawn(async move {
    if let Err(err) = runner.run().await {
      error!("{err:#}");
    }
  });

  Ok(NetworkNode {
    peer_id,
    command_tx,
  })
}

pub async fn submit_task_to_scheduler(
  scheduler: PeerAddress,
  task: DistributedTask,
) -> Result<TaskResponse> {
  let mut swarm = build_swarm()?;
  swarm.add_peer_address(scheduler.peer_id, scheduler.address.clone());

  let request_id = swarm
    .behaviour_mut()
    .request_response
    .send_request(&scheduler.peer_id, TaskRequest::Submit { task });

  let response = tokio::time::timeout(Duration::from_secs(20), async {
    loop {
      match swarm.select_next_some().await {
        SwarmEvent::Behaviour(TaskBehaviourEvent::RequestResponse(
          request_response::Event::Message { message, .. },
        )) => {
          if let request_response::Message::Response {
            request_id: received,
            response,
          } = message
          {
            if received == request_id {
              return Ok(response);
            }
          }
        }
        SwarmEvent::Behaviour(TaskBehaviourEvent::RequestResponse(
          request_response::Event::OutboundFailure {
            request_id: failed,
            error,
            ..
          },
        )) if failed == request_id => {
          bail!("submitting task failed: {error}");
        }
        SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
          if peer_id == Some(scheduler.peer_id) {
            bail!("connecting to scheduler failed: {error}");
          }
        }
        _ => {}
      }
    }
  })
  .await
  .context("timed out waiting for scheduler submit response")??;

  Ok(response)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkRole {
  Scheduler,
  Worker,
}

#[derive(Debug, Clone)]
pub struct PeerAddress {
  pub peer_id: PeerId,
  pub address: Multiaddr,
}

impl std::str::FromStr for PeerAddress {
  type Err = anyhow::Error;

  fn from_str(value: &str) -> Result<Self> {
    let (peer, addr) = value
      .split_once('@')
      .context("peer must be formatted as <peer_id>@<multiaddr>")?;
    Ok(Self {
      peer_id: peer.parse().context("parsing peer id")?,
      address: addr.parse().context("parsing multiaddr")?,
    })
  }
}

struct NetworkRunner {
  role: NetworkRole,
  swarm: Swarm<TaskBehaviour>,
  command_rx: mpsc::Receiver<NetworkCommand>,
  store: TaskStore,
  worker_tx: Option<mpsc::Sender<WorkerJob>>,
  workers: Vec<PeerId>,
  next_worker: usize,
  pending: HashMap<request_response::OutboundRequestId, DistributedTask>,
  backlog: VecDeque<DistributedTask>,
}

impl NetworkRunner {
  async fn run(mut self) -> Result<()> {
    let mut retry_tick = tokio::time::interval(Duration::from_secs(2));

    loop {
      tokio::select! {
          command = self.command_rx.recv() => {
              match command {
                  Some(NetworkCommand::Submit(task)) => self.submit_task(task),
                  None => return Ok(()),
              }
          }
          _ = retry_tick.tick(), if self.role == NetworkRole::Scheduler => {
              self.drain_backlog();
          }
          event = self.swarm.select_next_some() => {
              self.handle_swarm_event(event).await?;
          }
      }
    }
  }

  fn submit_task(&mut self, task: DistributedTask) {
    if self.workers.is_empty() {
      warn!(task_id = %task.id, "no worker connected yet; queueing task locally");
      self.backlog.push_back(task);
      return;
    }

    self.dispatch(task);
  }

  fn drain_backlog(&mut self) {
    if self.workers.is_empty() || self.backlog.is_empty() {
      return;
    }

    let count = self.backlog.len();
    for _ in 0 .. count {
      let Some(task) = self.backlog.pop_front() else {
        break;
      };
      self.dispatch(task);
    }
  }

  fn dispatch(&mut self, task: DistributedTask) {
    if self.workers.is_empty() {
      self.backlog.push_back(task);
      return;
    }

    let peer = self.workers[self.next_worker % self.workers.len()];
    self.next_worker = self.next_worker.wrapping_add(1);

    if let Err(err) = self.store.put_status(
      &task,
      crate::model::TaskStatus::Assigned,
      Some(peer.to_string()),
      None,
    ) {
      error!(task_id = %task.id, "{err:#}");
    }

    let request_id = self
      .swarm
      .behaviour_mut()
      .request_response
      .send_request(&peer, TaskRequest::Run { task: task.clone() });

    info!(task_id = %task.id, %peer, %request_id, "dispatched task");
    self.pending.insert(request_id, task);
  }

  async fn handle_swarm_event(&mut self, event: SwarmEvent<TaskBehaviourEvent>) -> Result<()> {
    match event {
      SwarmEvent::NewListenAddr { address, .. } => {
        let local = self.swarm.local_peer_id();
        info!(%local, %address, "listening");
        info!("share this as bootnode: {local}@{address}");
      }
      SwarmEvent::ConnectionEstablished { peer_id, .. } => {
        info!(%peer_id, "peer connected");
        if self.role == NetworkRole::Worker {
          let request_id = self
            .swarm
            .behaviour_mut()
            .request_response
            .send_request(&peer_id, TaskRequest::RegisterWorker);
          debug!(%peer_id, %request_id, "sent worker registration");
        }
      }
      SwarmEvent::ConnectionClosed { peer_id, .. } => {
        if self.role == NetworkRole::Scheduler {
          self.workers.retain(|id| id != &peer_id);
        }
        info!(%peer_id, "peer disconnected");
      }
      SwarmEvent::Behaviour(TaskBehaviourEvent::RequestResponse(event)) => {
        self.handle_request_response(event).await?;
      }
      other => {
        debug!(?other, "libp2p event");
      }
    }
    Ok(())
  }

  async fn handle_request_response(
    &mut self,
    event: request_response::Event<TaskRequest, TaskResponse>,
  ) -> Result<()> {
    match event {
      request_response::Event::Message { peer, message, .. } => match message {
        request_response::Message::Request {
          request, channel, ..
        } => match request {
          TaskRequest::RegisterWorker => self.handle_worker_registration(peer, channel),
          TaskRequest::Submit { task } => self.handle_submission(peer, task, channel),
          TaskRequest::Run { task } => self.handle_inbound_task(peer, task, channel).await?,
        },
        request_response::Message::Response {
          request_id,
          response,
        } => {
          self.handle_task_response(request_id, response);
        }
      },
      request_response::Event::OutboundFailure {
        peer,
        request_id,
        error,
        ..
      } => {
        warn!(%peer, %request_id, ?error, "task request failed");
        self.workers.retain(|id| id != &peer);
        if let Some(task) = self.pending.remove(&request_id) {
          self.backlog.push_back(task);
        }
      }
      request_response::Event::InboundFailure {
        peer,
        request_id,
        error,
        ..
      } => {
        warn!(%peer, %request_id, ?error, "inbound task response failed");
      }
      request_response::Event::ResponseSent {
        peer, request_id, ..
      } => {
        debug!(%peer, %request_id, "task response sent");
      }
    }
    Ok(())
  }

  fn handle_worker_registration(
    &mut self,
    peer: PeerId,
    channel: request_response::ResponseChannel<TaskResponse>,
  ) {
    let response = if self.role == NetworkRole::Scheduler {
      if !self.workers.contains(&peer) {
        self.workers.push(peer);
      }
      self.drain_backlog();
      TaskResponse::accepted(
        WORKER_REGISTRATION_TASK_ID.to_string(),
        "worker registered".to_string(),
        self.swarm.local_peer_id().to_string(),
      )
    } else {
      TaskResponse::rejected(
        WORKER_REGISTRATION_TASK_ID.to_string(),
        "this node is not a scheduler".to_string(),
        self.swarm.local_peer_id().to_string(),
      )
    };

    let _ = self
      .swarm
      .behaviour_mut()
      .request_response
      .send_response(channel, response);
  }

  fn handle_submission(
    &mut self,
    peer: PeerId,
    task: DistributedTask,
    channel: request_response::ResponseChannel<TaskResponse>,
  ) {
    let local_peer_id = self.swarm.local_peer_id().to_string();
    let response = if self.role == NetworkRole::Scheduler {
      match self.store.put_status(
        &task,
        crate::model::TaskStatus::Created,
        None,
        Some(format!("submitted by {peer}")),
      ) {
        Ok(()) => {
          info!(task_id = %task.id, %peer, "accepted submitted task");
          self.submit_task(task.clone());
          TaskResponse::accepted(task.id, "submitted to scheduler".to_string(), local_peer_id)
        }
        Err(err) => {
          error!(task_id = %task.id, "{err:#}");
          TaskResponse::rejected(
            task.id,
            format!("failed to persist task: {err}"),
            local_peer_id,
          )
        }
      }
    } else {
      TaskResponse::rejected(
        task.id,
        "this node is not a scheduler".to_string(),
        local_peer_id,
      )
    };

    let _ = self
      .swarm
      .behaviour_mut()
      .request_response
      .send_response(channel, response);
  }

  async fn handle_inbound_task(
    &mut self,
    peer: PeerId,
    task: DistributedTask,
    channel: request_response::ResponseChannel<TaskResponse>,
  ) -> Result<()> {
    if self.role != NetworkRole::Worker {
      let response = TaskResponse::rejected(
        task.id,
        "this node is not running a worker".to_string(),
        self.swarm.local_peer_id().to_string(),
      );
      let _ = self
        .swarm
        .behaviour_mut()
        .request_response
        .send_response(channel, response);
      return Ok(());
    }

    let Some(worker_tx) = &self.worker_tx else {
      let response = TaskResponse::rejected(
        task.id,
        "this node is not running a worker".to_string(),
        self.swarm.local_peer_id().to_string(),
      );
      let _ = self
        .swarm
        .behaviour_mut()
        .request_response
        .send_response(channel, response);
      return Ok(());
    };

    info!(task_id = %task.id, %peer, "received task");
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    if worker_tx
      .send(WorkerJob {
        task: task.clone(),
        reply: SharedReply::new(reply_tx),
      })
      .await
      .is_err()
    {
      let response = TaskResponse::rejected(
        task.id,
        "worker queue closed".to_string(),
        self.swarm.local_peer_id().to_string(),
      );
      let _ = self
        .swarm
        .behaviour_mut()
        .request_response
        .send_response(channel, response);
      return Ok(());
    }

    let response = match reply_rx.await {
      Ok(response) => response,
      Err(_) => TaskResponse::rejected(
        task.id,
        "worker dropped task response".to_string(),
        self.swarm.local_peer_id().to_string(),
      ),
    };

    if self
      .swarm
      .behaviour_mut()
      .request_response
      .send_response(channel, response)
      .is_err()
    {
      warn!("response channel closed before worker result was sent");
    }

    Ok(())
  }

  fn handle_task_response(
    &mut self,
    request_id: request_response::OutboundRequestId,
    response: TaskResponse,
  ) {
    if response.task_id == WORKER_REGISTRATION_TASK_ID {
      if response.accepted {
        info!(scheduler = %response.worker, "worker registration acknowledged");
      } else {
        warn!(
            scheduler = %response.worker,
            output = %response.output,
            "worker registration rejected"
        );
      }
      return;
    }

    let task = self.pending.remove(&request_id);
    let Some(task) = task else {
      warn!(%request_id, task_id = %response.task_id, "received response for unknown task");
      return;
    };

    let status = if response.accepted {
      crate::model::TaskStatus::Completed
    } else {
      crate::model::TaskStatus::Failed
    };

    if let Err(err) = self.store.update_with_output(
      &task,
      status,
      Some(response.worker.clone()),
      response.output.clone(),
    ) {
      error!(task_id = %task.id, "{err:#}");
    }

    info!(
        task_id = %task.id,
        worker = %response.worker,
        accepted = response.accepted,
        output = %response.output,
        "task finished"
    );
  }
}

fn build_swarm() -> Result<Swarm<TaskBehaviour>> {
  let behaviour = |_: &libp2p::identity::Keypair| {
    let protocols = [(StreamProtocol::new(TASK_PROTOCOL), ProtocolSupport::Full)];
    let request_response = request_response::json::Behaviour::new(
      protocols,
      request_response::Config::default().with_request_timeout(Duration::from_secs(60)),
    );

    Ok(TaskBehaviour { request_response })
  };

  let swarm = SwarmBuilder::with_new_identity()
    .with_tokio()
    .with_tcp(
      tcp::Config::default(),
      noise::Config::new,
      yamux::Config::default,
    )
    .context("building TCP transport")?
    .with_behaviour(behaviour)
    .context("building libp2p behaviour")?
    .with_swarm_config(|config| config.with_idle_connection_timeout(Duration::from_secs(3600)))
    .build();

  Ok(swarm)
}
