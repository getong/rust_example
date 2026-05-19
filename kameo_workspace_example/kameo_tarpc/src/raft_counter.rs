use std::{collections::BTreeMap, path::Path, sync::Arc, time::Duration};

use anyhow::{Context as _, anyhow};
use openraft::{
  BasicNode, Config, Raft, RaftNetworkFactory,
  async_runtime::WatchReceiver,
  error::{ReplicationClosed, StreamingError, Unreachable},
  network::{RPCOption, v2::RaftNetworkV2},
};
use openraft_rocksstore_crud::{RocksStateMachine, TypeConfig};
use tokio::sync::Mutex;
use tracing::info;
use types_kv::Request;

pub type NodeId = u64;
pub type CounterRaft = Raft<TypeConfig, RocksStateMachine>;

#[derive(Clone)]
pub struct CounterRaftHandle {
  raft: CounterRaft,
  cached_total: Arc<Mutex<i64>>,
}

impl CounterRaftHandle {
  pub async fn open_single_node(
    node_id: NodeId,
    db_path: impl AsRef<Path>,
  ) -> anyhow::Result<Self> {
    let config = Arc::new(
      Config {
        cluster_name: "kameo-tarpc-counter".to_string(),
        heartbeat_interval: 500,
        election_timeout_min: 1_500,
        election_timeout_max: 3_000,
        ..Default::default()
      }
      .validate()
      .context("invalid openraft config")?,
    );

    let (log_store, state_machine) = openraft_rocksstore_crud::new::<TypeConfig, _>(db_path)
      .await
      .context("open rocksdb-backed raft store")?;

    let raft = Raft::new(
      node_id,
      config,
      LoopbackNetworkFactory,
      log_store,
      state_machine.clone(),
    )
    .await
    .map_err(|err| anyhow!("failed to create raft: {err}"))?;

    maybe_initialize_single_node(&raft, node_id).await?;
    wait_for_leader(&raft, node_id).await?;
    let initial_total = read_counter_total_from_state_machine(&state_machine).await?;

    Ok(Self {
      raft,
      cached_total: Arc::new(Mutex::new(initial_total)),
    })
  }

  pub async fn add_and_get_total(
    &self,
    amount: u32,
    caller: String,
  ) -> anyhow::Result<CounterReply> {
    let key = COUNTER_KEY.to_string();
    let previous_total = self.current_total().await?;
    let new_total = previous_total + i64::from(amount);

    let response = self
      .raft
      .client_write(Request::Set {
        key: key.clone(),
        value: new_total.to_string(),
      })
      .await
      .map_err(|err| anyhow!("raft client_write failed: {err}"))?;

    let persisted = response.data.value.unwrap_or_else(|| new_total.to_string());
    let total = persisted
      .parse::<i64>()
      .map_err(|err| anyhow!("failed to parse persisted counter value '{persisted}': {err}"))?;

    info!(
      "raft persisted counter key={} caller={} amount={} total={}",
      key, caller, amount, total
    );

    let mut cached_total = self.cached_total.lock().await;
    *cached_total = total;

    Ok(CounterReply { total, caller })
  }

  pub async fn current_total(&self) -> anyhow::Result<i64> {
    let cached_total = self.cached_total.lock().await;
    Ok(*cached_total)
  }
}

#[derive(Debug, Clone)]
pub struct CounterReply {
  pub total: i64,
  pub caller: String,
}

const COUNTER_KEY: &str = "counter.total";

async fn maybe_initialize_single_node(raft: &CounterRaft, node_id: NodeId) -> anyhow::Result<()> {
  if raft
    .is_initialized()
    .await
    .map_err(|err| anyhow!("failed to query raft initialization: {err}"))?
  {
    return Ok(());
  }

  let mut nodes = BTreeMap::new();
  nodes.insert(
    node_id,
    BasicNode {
      addr: format!("node-{node_id}"),
    },
  );

  match raft.initialize(nodes).await {
    Ok(_) => Ok(()),
    Err(err) => {
      let err_text = err.to_string();
      if err_text.contains("initialized") || err_text.contains("not allowed") {
        Ok(())
      } else {
        Err(anyhow!("failed to initialize single-node raft: {err}"))
      }
    }
  }
}

async fn wait_for_leader(raft: &CounterRaft, node_id: NodeId) -> anyhow::Result<()> {
  for _ in 0 .. 50 {
    let metrics = raft.metrics().borrow_watched().clone();
    if metrics.current_leader == Some(node_id) {
      return Ok(());
    }
    tokio::time::sleep(Duration::from_millis(100)).await;
  }

  Err(anyhow!(
    "timed out waiting for single-node raft leader election"
  ))
}

async fn read_counter_total_from_state_machine(
  state_machine: &RocksStateMachine,
) -> anyhow::Result<i64> {
  let kvs = state_machine.kvs();
  let guard = kvs.read().await;
  match guard.get(COUNTER_KEY) {
    Some(value) => value
      .parse::<i64>()
      .map_err(|err| anyhow!("failed to parse stored counter value '{value}': {err}")),
    None => Ok(0),
  }
}

#[derive(Clone, Default)]
struct LoopbackNetworkFactory;

impl RaftNetworkFactory<TypeConfig> for LoopbackNetworkFactory {
  type Network = LoopbackNetwork;

  async fn new_client(&mut self, target: NodeId, target_node: &BasicNode) -> Self::Network {
    LoopbackNetwork {
      target,
      target_addr: target_node.addr.clone(),
    }
  }
}

#[derive(Clone)]
struct LoopbackNetwork {
  target: NodeId,
  target_addr: String,
}

#[derive(Debug)]
struct LoopbackError(String);

impl std::fmt::Display for LoopbackError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.0)
  }
}

impl std::error::Error for LoopbackError {}

impl RaftNetworkV2<TypeConfig> for LoopbackNetwork {
  async fn append_entries(
    &mut self,
    _rpc: openraft::raft::AppendEntriesRequest<TypeConfig>,
    _option: RPCOption,
  ) -> Result<
    openraft::raft::AppendEntriesResponse<TypeConfig>,
    openraft::error::RPCError<TypeConfig>,
  > {
    Err(openraft::error::RPCError::Unreachable(Unreachable::new(
      &LoopbackError(format!(
        "single-node loopback append_entries target={} addr={}",
        self.target, self.target_addr
      )),
    )))
  }

  async fn vote(
    &mut self,
    _rpc: openraft::raft::VoteRequest<TypeConfig>,
    _option: RPCOption,
  ) -> Result<openraft::raft::VoteResponse<TypeConfig>, openraft::error::RPCError<TypeConfig>> {
    Err(openraft::error::RPCError::Unreachable(Unreachable::new(
      &LoopbackError(format!(
        "single-node loopback vote target={} addr={}",
        self.target, self.target_addr
      )),
    )))
  }

  async fn full_snapshot(
    &mut self,
    _vote: openraft::type_config::alias::VoteOf<TypeConfig>,
    _snapshot: openraft::type_config::alias::SnapshotOf<TypeConfig>,
    _cancel: impl std::future::Future<Output = ReplicationClosed> + openraft::OptionalSend + 'static,
    _option: RPCOption,
  ) -> Result<openraft::raft::SnapshotResponse<TypeConfig>, StreamingError<TypeConfig>> {
    Err(StreamingError::Unreachable(Unreachable::new(
      &LoopbackError(format!(
        "single-node loopback full_snapshot target={} addr={}",
        self.target, self.target_addr
      )),
    )))
  }
}

pub type SharedCounterRaft = Arc<Mutex<CounterRaftHandle>>;
