use openraft::BasicNode;
use raft_kv_memstore_network_v2::store::Request;
use raft_kv_memstore_network_v2::{new_raft, router::Router, typ};
use std::collections::BTreeMap;
use std::time::Duration;
use tokio::{task, task::LocalSet};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
  tracing_subscriber::fmt()
    .with_target(true)
    .with_thread_ids(true)
    .with_level(true)
    .with_ansi(false)
    .with_env_filter(EnvFilter::from_default_env())
    .init();

  let router = Router::default();
  let local = LocalSet::new();

  // Move the creation of Raft nodes and their apps inside the LocalSet
  local
    .run_until(async move {
      let (raft1, app1) = new_raft(1, router.clone()).await;
      let (raft2, app2) = new_raft(2, router.clone()).await;

      task::spawn_local(app1.run());
      task::spawn_local(app2.run());

      let rafts = [raft1, raft2];

      // Run your test after spawning the local tasks
      run_test(&rafts, router).await;
    })
    .await;
}

async fn run_test(rafts: &[typ::Raft], router: Router) {
  let _ = router;

  // Wait for server to start up.
  tokio::time::sleep(Duration::from_millis(200)).await;

  let raft1 = &rafts[0];
  let raft2 = &rafts[1];

  println!("=== init single node cluster");
  {
    let mut nodes = BTreeMap::new();
    nodes.insert(
      1,
      BasicNode {
        addr: "".to_string(),
      },
    );
    raft1.initialize(nodes).await.unwrap();
  }

  println!("=== write 2 logs");
  {
    let resp = raft1
      .client_write(Request::set("foo1", "bar1"))
      .await
      .unwrap();
    println!("write resp: {:#?}", resp);
    let resp = raft1
      .client_write(Request::set("foo2", "bar2"))
      .await
      .unwrap();
    println!("write resp: {:#?}", resp);
  }

  println!("=== let node-1 take a snapshot");
  {
    raft1.trigger().snapshot().await.unwrap();

    // Wait for a while to let the snapshot get done.
    tokio::time::sleep(Duration::from_millis(500)).await;
  }

  println!("=== metrics after building snapshot");
  {
    let metrics = raft1.metrics().borrow().clone();
    println!("node 1 metrics: {:#?}", metrics);
    assert_eq!(Some(3), metrics.snapshot.map(|x| x.index));
    assert_eq!(Some(3), metrics.purged.map(|x| x.index));
  }

  println!("=== add-learner node-2");
  {
    let node = BasicNode {
      addr: "".to_string(),
    };
    let resp = raft1.add_learner(2, node, true).await.unwrap();
    println!("add-learner node-2 resp: {:#?}", resp);
  }

  // Wait for a while to let the node 2 to receive snapshot replication.
  tokio::time::sleep(Duration::from_millis(500)).await;

  println!("=== metrics of node 2 that received snapshot");
  {
    let metrics = raft2.metrics().borrow().clone();
    println!("node 2 metrics: {:#?}", metrics);
    assert_eq!(Some(3), metrics.snapshot.map(|x| x.index));
    assert_eq!(Some(3), metrics.purged.map(|x| x.index));
  }

  // In this example, the snapshot is just a copy of the state machine.
  let snapshot = raft2.get_snapshot().await.unwrap();
  println!("node 2 received snapshot: {:#?}", snapshot);
}
