use std::{net::SocketAddr, sync::Arc};

use anyhow::{Result, anyhow};
use axum::{Extension, Json, routing::post};
use axum_macros::debug_handler;
use clap::Parser;
use tokio::sync::Mutex;

#[derive(Parser, Debug)]
struct Args {
  #[arg(long)]
  id: u64,
}

#[tokio::main]
async fn main() {
  let args = Args::parse();

  let nodes = ["0.0.0.0:8000", "0.0.0.0:8001", "0.0.0.0:8002"];

  let current_addr = nodes[args.id as usize];

  let nodes: Vec<(u64, &'static str)> = nodes
    .into_iter()
    .enumerate()
    .filter(|(id, _)| *id as u64 != args.id)
    .map(|(id, addr)| (id as u64, addr))
    .collect();

  let config = Config::new(args.id, nodes);

  let paxos = Paxos::new(config);

  let router = axum::Router::new()
    .route("/", post(client_propose))
    .route(
      "/acceptor/handle-prepare-message",
      post(handle_prepare_message),
    )
    .route("/acceptor/handle-propose", post(handle_propose))
    .layer(Extension(Arc::new(Mutex::new(paxos))));

  let addr: SocketAddr = current_addr.parse().unwrap();
  println!("listening on {addr}");

  let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
  axum::serve(listener, router).await.unwrap();
}

#[debug_handler]
async fn client_propose(
  Extension(paxos): Extension<Arc<Mutex<Paxos>>>,
  value: String,
) -> Result<String, String> {
  println!("Received propose with value<{value}>");

  let mut paxos = paxos.lock().await;

  paxos
    .proposer
    .prepare(value.clone())
    .await
    .map_err(|err| err.to_string())?;

  Ok(format!("Accepted {value}"))
}

#[debug_handler]
async fn handle_prepare_message(
  Extension(paxos): Extension<Arc<Mutex<Paxos>>>,
  prepare_id: String,
) -> Result<Json<Promise>, String> {
  let mut paxos = paxos.lock().await;

  match paxos
    .acceptor
    .handle_prepare(prepare_id.parse::<u64>().map_err(|err| err.to_string())?)
  {
    Ok(promise) => Ok(Json(promise)),
    Err(err) => Err(err.to_string()),
  }
}

async fn handle_propose(
  Extension(paxos): Extension<Arc<Mutex<Paxos>>>,
  Json(propose): Json<Propose>,
) -> Result<Json<Value>, String> {
  let mut paxos = paxos.lock().await;

  match paxos.acceptor.handle_propose(propose) {
    Ok(value) => Ok(Json(value)),
    Err(err) => Err(err.to_string()),
  }
}

#[derive(Clone, Debug)]
pub struct Node {
  pub id: u64,
  pub addr: SocketAddr,
}

impl Node {
  fn new(id: u64, addr: SocketAddr) -> Self {
    Self { id, addr }
  }

  fn endpoint(&self, path: &str) -> String {
    format!("http://{}/{}", self.addr, path.trim_start_matches('/'))
  }
}

#[derive(Clone, Debug)]
pub struct Config {
  pub id: u64,
  pub nodes: Vec<Node>,
}

impl Config {
  fn new(id: u64, addresses: Vec<(u64, &'static str)>) -> Self {
    let mut nodes = Vec::new();

    for (id, addr) in addresses {
      nodes.push(Node::new(id, addr.parse().unwrap()));
    }

    Self { id, nodes }
  }
}

struct Paxos {
  pub proposer: Proposer,
  pub acceptor: Acceptor,
}

impl Paxos {
  fn new(config: Config) -> Self {
    Self {
      proposer: Proposer::new(config),
      acceptor: Acceptor::default(),
    }
  }
}

pub struct Proposer {
  pub id: u64,
  pub config: Config,
  pub promises: Vec<Promise>,
}

impl Proposer {
  pub fn new(config: Config) -> Self {
    Self {
      id: 0,
      config,
      promises: Vec::new(),
    }
  }
}

impl Proposer {
  pub async fn prepare(&mut self, value: String) -> anyhow::Result<()> {
    self.id += 1;

    let client = reqwest::Client::new();

    let reqs = self.config.nodes.iter().map(|node| {
      client
        .post(node.endpoint("/acceptor/handle-prepare-message"))
        .json(&self.id)
        .send()
    });

    let result = futures::future::join_all(reqs).await;

    let mut promises = Vec::with_capacity(result.len());

    for result in result.into_iter().flatten() {
      promises.push(result.json::<Promise>().await?)
    }

    let majority = (self.config.nodes.len() / 2) + 1;

    if promises.len() + 1 < majority {
      return Err(anyhow!(
        "didn't received promise from majority of acceptors"
      ));
    }

    let accepted_promise = promises
      .into_iter()
      .filter(|Promise(value)| value.value.is_some())
      .max_by_key(|Promise(value)| value.id);

    let has_accepted_value = accepted_promise.is_some();

    let value = match accepted_promise {
      Some(value) => value.0.value.unwrap(),
      None => value,
    };

    let propose = Propose(Value {
      id: self.id,
      value: Some(value),
    });

    let requests = self.config.nodes.iter().map(|node| {
      client
        .post(node.endpoint("/acceptor/handle-propose"))
        .json(&propose)
        .send()
    });

    let responses = futures::future::join_all(requests).await;

    let mut accepted_values = Vec::with_capacity(responses.len());

    for result in responses.into_iter().flatten() {
      accepted_values.push(result.json::<Value>().await?);
    }

    if accepted_values.len() + 1 < majority {
      return Err(anyhow!("value not accepted by majority"));
    }

    if has_accepted_value {
      return Err(anyhow!("already accepted another value"));
    }

    Ok(())
  }
}

#[derive(Clone, serde::Deserialize, serde::Serialize)]
pub struct Propose(pub Value);

#[derive(Default)]
pub struct Acceptor {
  max_id: u64,
  accepted_propose: Option<Propose>,
}

impl Acceptor {
  pub fn handle_prepare(&mut self, prepare_id: u64) -> Result<Promise> {
    if prepare_id < self.max_id {
      return Err(anyhow!("already accepted a propose with a higher id"));
    }

    self.max_id = prepare_id;

    if self.accepted_propose.is_some() {
      let value = self.accepted_propose.clone().unwrap().0.value;

      return Ok(Promise(Value {
        id: prepare_id,
        value,
      }));
    }

    Ok(Promise(Value {
      id: prepare_id,
      value: None,
    }))
  }

  pub fn handle_propose(&mut self, propose: Propose) -> Result<Value> {
    if self.max_id != propose.0.id {
      return Err(anyhow!("cannot accept propose with lower id"));
    }

    self.accepted_propose = Some(propose.clone());

    Ok(propose.0)
  }
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Promise(pub Value);

#[derive(Clone, serde::Deserialize, serde::Serialize)]
pub struct Value {
  pub id: u64,
  pub value: Option<String>,
}
