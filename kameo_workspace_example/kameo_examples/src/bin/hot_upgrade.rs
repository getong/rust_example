use kameo::{
  Actor, Reply,
  actor::Spawn,
  message::{Context, Message},
};

// Rust cannot replace already compiled functions the way the BEAM can.
// This example shows the actor-level part of an Erlang-style hot upgrade:
// keep the same ActorRef alive, migrate state, then route new messages to new behavior.
#[derive(Actor)]
struct HotCounter {
  state: CounterState,
}

impl HotCounter {
  fn new() -> Self {
    Self {
      state: CounterState::V1(CounterV1State {
        total: 0,
        operations: 0,
      }),
    }
  }

  fn add(&mut self, amount: i64) -> CounterSnapshot {
    match &mut self.state {
      CounterState::V1(state) => {
        state.total += amount;
        state.operations += 1;
      }
      CounterState::V2(state) => {
        let applied = amount * state.multiplier;
        state.total += applied;
        state.operations += 1;
        state
          .history
          .push(format!("added {amount} x {} = {applied}", state.multiplier));

        if state.history.len() > 4 {
          state.history.remove(0);
        }
      }
    }

    self.snapshot()
  }

  fn upgrade_to_v2(&mut self, multiplier: i64) -> CounterSnapshot {
    let previous = std::mem::replace(
      &mut self.state,
      CounterState::V1(CounterV1State {
        total: 0,
        operations: 0,
      }),
    );

    self.state = match previous {
      CounterState::V1(state) => CounterState::V2(CounterV2State {
        total: state.total,
        operations: state.operations,
        multiplier,
        history: vec![format!(
          "migrated from v1 after {} operations",
          state.operations
        )],
      }),
      CounterState::V2(mut state) => {
        state.multiplier = multiplier;
        state
          .history
          .push(format!("changed multiplier to {multiplier}"));
        CounterState::V2(state)
      }
    };

    self.snapshot()
  }

  fn snapshot(&self) -> CounterSnapshot {
    match &self.state {
      CounterState::V1(state) => CounterSnapshot {
        version: "v1".to_string(),
        total: state.total,
        operations: state.operations,
        multiplier: None,
        history: Vec::new(),
      },
      CounterState::V2(state) => CounterSnapshot {
        version: "v2".to_string(),
        total: state.total,
        operations: state.operations,
        multiplier: Some(state.multiplier),
        history: state.history.clone(),
      },
    }
  }
}

enum CounterState {
  V1(CounterV1State),
  V2(CounterV2State),
}

struct CounterV1State {
  total: i64,
  operations: u64,
}

struct CounterV2State {
  total: i64,
  operations: u64,
  multiplier: i64,
  history: Vec<String>,
}

struct Add {
  amount: i64,
}

impl Message<Add> for HotCounter {
  type Reply = CounterSnapshot;

  async fn handle(&mut self, msg: Add, _ctx: &mut Context<Self, Self::Reply>) -> Self::Reply {
    self.add(msg.amount)
  }
}

struct UpgradeToV2 {
  multiplier: i64,
}

impl Message<UpgradeToV2> for HotCounter {
  type Reply = CounterSnapshot;

  async fn handle(
    &mut self,
    msg: UpgradeToV2,
    _ctx: &mut Context<Self, Self::Reply>,
  ) -> Self::Reply {
    self.upgrade_to_v2(msg.multiplier)
  }
}

struct GetSnapshot;

impl Message<GetSnapshot> for HotCounter {
  type Reply = CounterSnapshot;

  async fn handle(
    &mut self,
    _msg: GetSnapshot,
    _ctx: &mut Context<Self, Self::Reply>,
  ) -> Self::Reply {
    self.snapshot()
  }
}

#[derive(Debug, Clone, Reply)]
struct CounterSnapshot {
  version: String,
  total: i64,
  operations: u64,
  multiplier: Option<i64>,
  history: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let counter = HotCounter::spawn(HotCounter::new());
  let actor_id = counter.id();

  let before = counter.ask(Add { amount: 10 }).await?;
  println!("before upgrade: {before:#?}");

  let upgraded = counter.ask(UpgradeToV2 { multiplier: 3 }).await?;
  println!("after upgrade: {upgraded:#?}");

  let after = counter.ask(Add { amount: 10 }).await?;
  println!("after v2 add: {after:#?}");

  assert_eq!(actor_id, counter.id());
  assert_eq!(before.version, "v1");
  assert_eq!(upgraded.version, "v2");
  assert_eq!(after.total, 40);
  assert_eq!(after.operations, 2);
  assert_eq!(after.multiplier, Some(3));
  assert_eq!(after.history.len(), 2);

  let snapshot = counter.ask(GetSnapshot).await?;
  println!("same actor {actor_id} is now running {}", snapshot.version);

  counter.stop_gracefully().await?;
  counter.wait_for_shutdown().await;

  Ok(())
}
