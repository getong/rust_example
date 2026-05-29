use std::{
  sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
  },
  time::Duration,
};

use kameo::{
  Actor, Reply,
  actor::Spawn,
  message::{Context, Message},
};

static CODE_GENERATION: AtomicU64 = AtomicU64::new(0);

#[derive(Actor)]
struct HotPatchCounter {
  state: CounterState,
}

impl HotPatchCounter {
  fn new() -> Self {
    Self {
      state: CounterState::V1(CounterV1State {
        total: 0,
        operations: 0,
        code_generation: CODE_GENERATION.load(Ordering::SeqCst),
      }),
    }
  }

  fn handle_command(&mut self, command: CounterCommand) -> CounterSnapshot {
    self.sync_code_generation();

    match command {
      CounterCommand::Add(amount) => self.add(amount),
      CounterCommand::GetSnapshot => self.snapshot(),
      CounterCommand::SimulatePatch => self.simulate_patch(),
    }
  }

  fn sync_code_generation(&mut self) {
    let current_generation = CODE_GENERATION.load(Ordering::SeqCst);
    let previous_generation = self.state.code_generation();
    if previous_generation == current_generation {
      return;
    }

    self.code_changed(previous_generation, current_generation);
    self.state.set_code_generation(current_generation);
  }

  fn simulate_patch(&mut self) -> CounterSnapshot {
    let new_generation = next_code_generation();
    let old_generation = self.state.code_generation();
    println!("simulated subsecond patch: code generation {new_generation}");

    self.code_changed(old_generation, new_generation);
    self.state.set_code_generation(new_generation);
    self.snapshot()
  }

  fn code_changed(&mut self, old_generation: u64, new_generation: u64) {
    println!(
      "[counter@{}] code_change: generation {old_generation} -> {new_generation}",
      self.state.version()
    );

    match &mut self.state {
      CounterState::V1(state) => {
        self.state = CounterState::V2(CounterV2State {
          total: state.total,
          operations: state.operations,
          code_generation: new_generation,
          multiplier: 2,
          history: vec![format!(
            "migrated from v1 during code_change {old_generation}->{new_generation}"
          )],
        });
      }
      CounterState::V2(state) => {
        state.code_generation = new_generation;
        state.multiplier += 1;
        state.history.push(format!(
          "patched existing v2 {old_generation}->{new_generation}; multiplier={}",
          state.multiplier
        ));
        trim_history(&mut state.history);
      }
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
        trim_history(&mut state.history);
      }
    }

    self.snapshot()
  }

  fn snapshot(&self) -> CounterSnapshot {
    match &self.state {
      CounterState::V1(state) => CounterSnapshot {
        version: "v1".to_string(),
        total: state.total,
        operations: state.operations,
        code_generation: state.code_generation,
        multiplier: None,
        history: Vec::new(),
      },
      CounterState::V2(state) => CounterSnapshot {
        version: "v2".to_string(),
        total: state.total,
        operations: state.operations,
        code_generation: state.code_generation,
        multiplier: Some(state.multiplier),
        history: state.history.clone(),
      },
    }
  }
}

enum CounterCommand {
  Add(i64),
  GetSnapshot,
  SimulatePatch,
}

enum CounterState {
  V1(CounterV1State),
  V2(CounterV2State),
}

impl CounterState {
  fn version(&self) -> &'static str {
    match self {
      Self::V1(_) => "v1",
      Self::V2(_) => "v2",
    }
  }

  fn code_generation(&self) -> u64 {
    match self {
      Self::V1(state) => state.code_generation,
      Self::V2(state) => state.code_generation,
    }
  }

  fn set_code_generation(&mut self, generation: u64) {
    match self {
      Self::V1(state) => state.code_generation = generation,
      Self::V2(state) => state.code_generation = generation,
    }
  }
}

struct CounterV1State {
  total: i64,
  operations: u64,
  code_generation: u64,
}

struct CounterV2State {
  total: i64,
  operations: u64,
  code_generation: u64,
  multiplier: i64,
  history: Vec<String>,
}

struct Add {
  amount: i64,
}

impl Message<Add> for HotPatchCounter {
  type Reply = CounterSnapshot;

  async fn handle(&mut self, msg: Add, _ctx: &mut Context<Self, Self::Reply>) -> Self::Reply {
    subsecond::call(|| self.handle_command(CounterCommand::Add(msg.amount)))
  }
}

struct SimulatePatch;

impl Message<SimulatePatch> for HotPatchCounter {
  type Reply = CounterSnapshot;

  async fn handle(
    &mut self,
    _msg: SimulatePatch,
    _ctx: &mut Context<Self, Self::Reply>,
  ) -> Self::Reply {
    subsecond::call(|| self.handle_command(CounterCommand::SimulatePatch))
  }
}

struct GetSnapshot;

impl Message<GetSnapshot> for HotPatchCounter {
  type Reply = CounterSnapshot;

  async fn handle(
    &mut self,
    _msg: GetSnapshot,
    _ctx: &mut Context<Self, Self::Reply>,
  ) -> Self::Reply {
    subsecond::call(|| self.handle_command(CounterCommand::GetSnapshot))
  }
}

#[derive(Debug, Clone, Reply)]
struct CounterSnapshot {
  version: String,
  total: i64,
  operations: u64,
  code_generation: u64,
  multiplier: Option<i64>,
  history: Vec<String>,
}

fn install_subsecond_handler() {
  subsecond::register_handler(Arc::new(|| {
    let generation = next_code_generation();
    eprintln!("subsecond patch applied: code generation {generation}");
  }));
}

fn next_code_generation() -> u64 {
  CODE_GENERATION.fetch_add(1, Ordering::SeqCst) + 1
}

fn trim_history(history: &mut Vec<String>) {
  const MAX_HISTORY: usize = 5;

  if history.len() > MAX_HISTORY {
    history.drain(.. (history.len() - MAX_HISTORY));
  }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  install_subsecond_handler();

  let counter = HotPatchCounter::spawn(HotPatchCounter::new());
  let actor_id = counter.id();

  let before_patch = counter.ask(Add { amount: 10 }).await?;
  println!("before patch: {before_patch:#?}");

  let after_patch = counter.ask(SimulatePatch).await?;
  println!("after patch migration: {after_patch:#?}");

  let after_v2_add = counter.ask(Add { amount: 10 }).await?;
  println!("after v2 add: {after_v2_add:#?}");

  tokio::time::sleep(Duration::from_millis(10)).await;

  assert_eq!(actor_id, counter.id());
  assert_eq!(before_patch.version, "v1");
  assert_eq!(before_patch.total, 10);
  assert_eq!(after_patch.version, "v2");
  assert_eq!(after_patch.multiplier, Some(2));
  assert_eq!(after_patch.history.len(), 1);
  assert_eq!(after_v2_add.total, 30);
  assert_eq!(after_v2_add.operations, 2);
  assert_eq!(after_v2_add.code_generation, after_patch.code_generation);
  assert_eq!(after_v2_add.history.len(), 2);

  let snapshot = counter.ask(GetSnapshot).await?;
  println!(
    "same kameo actor {actor_id} now runs {} behavior behind a subsecond call boundary",
    snapshot.version
  );

  counter.stop_gracefully().await?;
  counter.wait_for_shutdown().await;

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn code_change_migrates_v1_state_to_v2() {
    let mut counter = HotPatchCounter {
      state: CounterState::V1(CounterV1State {
        total: 7,
        operations: 3,
        code_generation: 1,
      }),
    };

    counter.code_changed(1, 2);

    let snapshot = counter.snapshot();
    assert_eq!(snapshot.version, "v2");
    assert_eq!(snapshot.total, 7);
    assert_eq!(snapshot.operations, 3);
    assert_eq!(snapshot.code_generation, 2);
    assert_eq!(snapshot.multiplier, Some(2));
  }

  #[test]
  fn v2_behavior_uses_current_multiplier() {
    let mut counter = HotPatchCounter {
      state: CounterState::V2(CounterV2State {
        total: 10,
        operations: 1,
        code_generation: 2,
        multiplier: 3,
        history: Vec::new(),
      }),
    };

    let snapshot = counter.add(4);

    assert_eq!(snapshot.total, 22);
    assert_eq!(snapshot.operations, 2);
    assert_eq!(snapshot.multiplier, Some(3));
  }
}
