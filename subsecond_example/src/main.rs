use std::{
  io::{self, BufRead},
  sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
    mpsc::{self, RecvTimeoutError, Sender},
  },
  thread,
  time::Duration,
};

static CODE_GENERATION: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
enum ProcessState {
  V1(StateV1),
  V2(StateV2),
}

#[derive(Debug)]
struct StateV1 {
  counter: i64,
  ticks: u64,
  code_generation: u64,
}

#[derive(Debug)]
struct StateV2 {
  counter: i64,
  ticks: u64,
  code_generation: u64,
  total_delta: i64,
  last_event: String,
}

impl ProcessState {
  fn new() -> Self {
    Self::V1(StateV1 {
      counter: 0,
      ticks: 0,
      code_generation: CODE_GENERATION.load(Ordering::SeqCst),
    })
  }

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

#[derive(Clone, Copy, Debug)]
enum Message {
  Tick,
  Increment(i64),
  Decrement(i64),
  Status,
  Upgrade,
  SimulatePatch,
  Help,
  Quit,
}

fn main() {
  subsecond::register_handler(Arc::new(|| {
    let generation = next_code_generation();
    eprintln!("subsecond patch applied: code generation {generation}");
  }));

  let (sender, receiver) = mpsc::channel();
  thread::spawn(move || read_stdin(sender));

  let mut state = ProcessState::new();

  println!("subsecond actor hot-code example");
  print_help();

  loop {
    let message = match receiver.recv_timeout(Duration::from_secs(2)) {
      Ok(message) => message,
      Err(RecvTimeoutError::Timeout) => Message::Tick,
      Err(RecvTimeoutError::Disconnected) => Message::Quit,
    };

    let keep_running = subsecond::call(|| process_message(&mut state, message));
    if !keep_running {
      break;
    }
  }

  println!("actor stopped with state: {state:?}");
}

fn read_stdin(sender: Sender<Message>) {
  for line in io::stdin().lock().lines() {
    let Ok(line) = line else {
      continue;
    };

    let message = parse_command(line.trim());
    let should_quit = matches!(message, Message::Quit);

    if sender.send(message).is_err() || should_quit {
      break;
    }
  }
}

fn parse_command(input: &str) -> Message {
  let mut parts = input.split_whitespace();

  match parts.next() {
    Some("inc") | Some("+") => Message::Increment(parse_amount(parts.next())),
    Some("dec") | Some("-") => Message::Decrement(parse_amount(parts.next())),
    Some("status") | Some("s") => Message::Status,
    Some("upgrade") | Some("u") => Message::Upgrade,
    Some("patch") | Some("p") => Message::SimulatePatch,
    Some("help") | Some("?") | None => Message::Help,
    Some("quit") | Some("q") | Some("exit") => Message::Quit,
    Some(_) => Message::Help,
  }
}

fn parse_amount(value: Option<&str>) -> i64 {
  value.and_then(|value| value.parse().ok()).unwrap_or(1)
}

fn process_message(state: &mut ProcessState, message: Message) -> bool {
  sync_code_generation(state);

  match message {
    Message::Tick => handle_tick(state),
    Message::Increment(amount) => handle_increment(state, amount),
    Message::Decrement(amount) => handle_decrement(state, amount),
    Message::Status => print_status(state),
    Message::Upgrade => upgrade_to_v2(state, "manual upgrade command"),
    Message::SimulatePatch => simulate_patch(state),
    Message::Help => print_help(),
    Message::Quit => return false,
  }

  true
}

fn sync_code_generation(state: &mut ProcessState) {
  let current_generation = CODE_GENERATION.load(Ordering::SeqCst);
  let previous_generation = state.code_generation();
  if previous_generation == current_generation {
    return;
  }

  subsecond::call(|| code_changed(state, previous_generation, current_generation));
  state.set_code_generation(current_generation);
}

fn simulate_patch(state: &mut ProcessState) {
  let new_generation = next_code_generation();
  let old_generation = state.code_generation();
  println!("simulated patch applied: code generation {new_generation}");

  subsecond::call(|| code_changed(state, old_generation, new_generation));
  state.set_code_generation(new_generation);
}

fn next_code_generation() -> u64 {
  CODE_GENERATION.fetch_add(1, Ordering::SeqCst) + 1
}

fn actor_name() -> &'static str {
  "counter"
}

fn actor_label(state: &ProcessState) -> String {
  format!("{}@{}", actor_name(), state.version())
}

fn code_changed(state: &mut ProcessState, old_generation: u64, new_generation: u64) {
  println!(
    "[{}] code_change: generation {old_generation} -> {new_generation}",
    actor_label(state)
  );

  match state {
    ProcessState::V1(_) => upgrade_to_v2(state, "code_change after patch"),
    ProcessState::V2(state_v2) => {
      state_v2.last_event = format!("code_change generation {old_generation} -> {new_generation}");
    }
  }
}

fn upgrade_to_v2(state: &mut ProcessState, reason: &str) {
  let next_state = match state {
    ProcessState::V1(state_v1) => Some(StateV2 {
      counter: state_v1.counter,
      ticks: state_v1.ticks,
      code_generation: state_v1.code_generation,
      total_delta: 0,
      last_event: reason.to_string(),
    }),
    ProcessState::V2(state_v2) => {
      state_v2.last_event = format!("already upgraded: {reason}");
      println!("[{}] state is already V2", actor_name());
      None
    }
  };

  if let Some(next_state) = next_state {
    *state = ProcessState::V2(next_state);
    println!("[{}@v2] state upgraded from V1: {reason}", actor_name());
  }
}

fn handle_tick(state: &mut ProcessState) {
  match state {
    ProcessState::V1(state_v1) => {
      state_v1.ticks += 1;
      println!(
        "[{}@v1] heartbeat tick={} counter={}",
        actor_name(),
        state_v1.ticks,
        state_v1.counter
      );
    }
    ProcessState::V2(state_v2) => {
      state_v2.ticks += 1;
      state_v2.last_event = "tick".to_string();
      println!(
        "[{}@v2] heartbeat tick={} counter={} total_delta={}",
        actor_name(),
        state_v2.ticks,
        state_v2.counter,
        state_v2.total_delta
      );
    }
  }
}

fn handle_increment(state: &mut ProcessState, amount: i64) {
  match state {
    ProcessState::V1(state_v1) => {
      state_v1.counter += amount;
      println!(
        "[{}@v1] +{amount} => counter={}",
        actor_name(),
        state_v1.counter
      );
    }
    ProcessState::V2(state_v2) => {
      state_v2.counter += amount;
      state_v2.total_delta += amount;
      state_v2.last_event = format!("inc {amount}");
      println!(
        "[{}@v2] +{amount} => counter={} total_delta={}",
        actor_name(),
        state_v2.counter,
        state_v2.total_delta
      );
    }
  }
}

fn handle_decrement(state: &mut ProcessState, amount: i64) {
  match state {
    ProcessState::V1(state_v1) => {
      state_v1.counter -= amount;
      println!(
        "[{}@v1] -{amount} => counter={}",
        actor_name(),
        state_v1.counter
      );
    }
    ProcessState::V2(state_v2) => {
      state_v2.counter -= amount;
      state_v2.total_delta -= amount;
      state_v2.last_event = format!("dec {amount}");
      println!(
        "[{}@v2] -{amount} => counter={} total_delta={}",
        actor_name(),
        state_v2.counter,
        state_v2.total_delta
      );
    }
  }
}

fn print_status(state: &ProcessState) {
  match state {
    ProcessState::V1(state_v1) => {
      println!(
        "[{}@v1] status counter={} ticks={} code_generation={}",
        actor_name(),
        state_v1.counter,
        state_v1.ticks,
        state_v1.code_generation
      );
    }
    ProcessState::V2(state_v2) => {
      println!(
        "[{}@v2] status counter={} ticks={} code_generation={} total_delta={} last_event={:?}",
        actor_name(),
        state_v2.counter,
        state_v2.ticks,
        state_v2.code_generation,
        state_v2.total_delta,
        state_v2.last_event
      );
    }
  }
}

fn print_help() {
  println!("commands: inc [n], dec [n], status, upgrade, patch, help, quit");
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn upgrade_preserves_v1_fields_and_adds_v2_fields() {
    let mut state = ProcessState::V1(StateV1 {
      counter: 7,
      ticks: 3,
      code_generation: 1,
    });

    upgrade_to_v2(&mut state, "test migration");

    let ProcessState::V2(state_v2) = state else {
      panic!("state should be upgraded to V2");
    };
    assert_eq!(state_v2.counter, 7);
    assert_eq!(state_v2.ticks, 3);
    assert_eq!(state_v2.code_generation, 1);
    assert_eq!(state_v2.total_delta, 0);
    assert_eq!(state_v2.last_event, "test migration");
  }

  #[test]
  fn v2_tracks_total_delta() {
    let mut state = ProcessState::V2(StateV2 {
      counter: 10,
      ticks: 0,
      code_generation: 2,
      total_delta: 0,
      last_event: "created".to_string(),
    });

    handle_increment(&mut state, 4);
    handle_decrement(&mut state, 2);

    let ProcessState::V2(state_v2) = state else {
      panic!("state should remain V2");
    };
    assert_eq!(state_v2.counter, 12);
    assert_eq!(state_v2.total_delta, 2);
    assert_eq!(state_v2.last_event, "dec 2");
  }
}
