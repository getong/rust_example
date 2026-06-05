#![cfg(target_arch = "wasm32")]

#[cfg(all(feature = "guest-v1", feature = "guest-v2"))]
compile_error!("enable exactly one of `guest-v1` or `guest-v2` for the wasm guest");

#[cfg(not(any(feature = "guest-v1", feature = "guest-v2")))]
compile_error!("enable either `guest-v1` or `guest-v2` for the wasm guest");

wit_bindgen::generate!({
  path: "src/wit",
  world: "actor-world",
});

#[cfg(feature = "guest-v1")]
use host_actor::ActorStateV1;
#[cfg(feature = "guest-v2")]
use host_actor::ActorStateV2;
use host_actor::{ActorMsg, ActorMsgKind, ActorResponse, ActorState, GuestMessage};

use crate::demo::actor::host_actor;

const LOOP_SLEEP_MILLIS: i32 = 500;
const WASM_TO_HOST_INTERVAL_MILLIS: i32 = 3_000;
#[cfg(feature = "guest-v1")]
const V1_SCHEMA: u32 = 1;
#[cfg(feature = "guest-v2")]
const V2_SCHEMA: u32 = 2;

struct WasmActor;

impl exports::wasm_actor::Guest for WasmActor {
  fn handle_call(msgs: Vec<ActorMsg>, state: ActorState) -> ActorState {
    active_actor().handle_call(msgs, state)
  }

  fn migrate_state(state: ActorState) -> ActorState {
    active_actor().migrate_state(state)
  }

  fn state_schema() -> u32 {
    active_actor().state_schema()
  }

  fn render_state(state: ActorState) -> String {
    active_actor().render_state(state)
  }
}

trait GuestActor {
  fn handle_call(&self, msgs: Vec<ActorMsg>, state: ActorState) -> ActorState;
  fn migrate_state(&self, state: ActorState) -> ActorState;
  fn state_schema(&self) -> u32;
  fn render_state(&self, state: ActorState) -> String;
}

#[cfg(feature = "guest-v1")]
fn active_actor() -> impl GuestActor {
  GuestV1
}

#[cfg(feature = "guest-v2")]
fn active_actor() -> impl GuestActor {
  GuestV2
}

#[cfg(feature = "guest-v1")]
struct GuestV1;

#[cfg(feature = "guest-v1")]
impl GuestActor for GuestV1 {
  fn handle_call(&self, msgs: Vec<ActorMsg>, state: ActorState) -> ActorState {
    let ActorState::V1(mut state) = state else {
      panic!("guest-v1 handle-call requires actor-state.v1");
    };

    for msg in msgs {
      state = match msg.kind {
        ActorMsgKind::Tick => handle_v1_tick(state),
        ActorMsgKind::HostMessage => {
          if let Some(msg) = msg.host_message {
            println!(
              "wasm actor v1: received host主动消息 #{} payload={}",
              msg.sequence, msg.payload
            );
          }
          state
        }
      };
    }

    println!("wasm actor v1: state={}", render_v1_state(&state));
    ActorState::V1(state)
  }

  fn migrate_state(&self, state: ActorState) -> ActorState {
    match state {
      ActorState::V1(state) => ActorState::V1(state),
      ActorState::V2(_) => panic!("guest-v1 cannot downgrade actor-state.v2"),
    }
  }

  fn state_schema(&self) -> u32 {
    V1_SCHEMA
  }

  fn render_state(&self, state: ActorState) -> String {
    let ActorState::V1(state) = state else {
      panic!("guest-v1 render-state requires actor-state.v1");
    };

    render_v1_state(&state)
  }
}

#[cfg(feature = "guest-v2")]
struct GuestV2;

#[cfg(feature = "guest-v2")]
impl GuestActor for GuestV2 {
  fn handle_call(&self, msgs: Vec<ActorMsg>, state: ActorState) -> ActorState {
    let ActorState::V2(mut state) = state else {
      panic!("guest-v2 handle-call requires actor-state.v2");
    };

    for msg in msgs {
      state = match msg.kind {
        ActorMsgKind::Tick => handle_v2_tick(state),
        ActorMsgKind::HostMessage => {
          if let Some(msg) = msg.host_message {
            println!(
              "wasm actor v2: received host主动消息 #{} payload={}",
              msg.sequence, msg.payload
            );
            state.host_messages_seen += 1;
            state.last_host_sequence = msg.sequence;
            state.last_host_payload = msg.payload;
          }
          state
        }
      };
    }

    println!("wasm actor v2: state={}", render_v2_state(&state));
    ActorState::V2(state)
  }

  fn migrate_state(&self, state: ActorState) -> ActorState {
    match state {
      ActorState::V2(state) => ActorState::V2(state),
      ActorState::V1(state) => ActorState::V2(ActorStateV2 {
        tick: state.tick,
        last_host_reply: state.last_host_reply,
        elapsed_since_push: state.elapsed_since_push,
        last_response: state.last_response,
        upgrade_generation: 1,
        migrated_from_tick: state.tick,
        host_messages_seen: 0,
        proactive_sends: 0,
        last_host_sequence: 0,
        last_host_payload: String::new(),
      }),
    }
  }

  fn state_schema(&self) -> u32 {
    V2_SCHEMA
  }

  fn render_state(&self, state: ActorState) -> String {
    let ActorState::V2(state) = state else {
      panic!("guest-v2 render-state requires actor-state.v2");
    };

    render_v2_state(&state)
  }
}

#[cfg(feature = "guest-v1")]
fn handle_v1_tick(mut state: ActorStateV1) -> ActorStateV1 {
  state.tick += 1;
  state.elapsed_since_push += LOOP_SLEEP_MILLIS;
  if state.elapsed_since_push >= WASM_TO_HOST_INTERVAL_MILLIS {
    state.elapsed_since_push = 0;
    let response = send_proactive_message(
      "wasm actor v1",
      state.tick,
      state.last_response.handled,
      state.last_host_reply,
    );
    state.last_host_reply = response.reply;
    state.last_response = response;
  }

  state
}

#[cfg(feature = "guest-v2")]
fn handle_v2_tick(mut state: ActorStateV2) -> ActorStateV2 {
  state.tick += 1;
  state.elapsed_since_push += LOOP_SLEEP_MILLIS;
  if state.elapsed_since_push >= WASM_TO_HOST_INTERVAL_MILLIS {
    state.elapsed_since_push = 0;
    let response = send_proactive_message(
      "wasm actor v2",
      state.tick,
      state.last_response.handled,
      state.last_host_reply,
    );
    state.last_host_reply = response.reply;
    state.last_response = response;
    state.proactive_sends += 1;
  }

  state
}

fn send_proactive_message(
  actor_label: &str,
  tick: i32,
  last_handled: u64,
  last_host_reply: i32,
) -> ActorResponse {
  let request = GuestMessage {
    tick,
    last_handled,
    last_host_reply,
    payload: format!("wasm主动消息 at tick {tick}"),
  };
  println!(
    "{actor_label}: actively sending message to host: tick={} payload={}",
    request.tick, request.payload
  );

  let response = host_actor::send_to_host(&request);
  println!(
    "{actor_label}: host reply handled #{} reply={} message={}",
    response.handled, response.reply, response.message
  );
  response
}

#[cfg(feature = "guest-v1")]
fn render_v1_state(state: &ActorStateV1) -> String {
  format!(
    r#"{{"schema":1,"tick":{},"handled":{},"reply":{},"message":{:?}}}"#,
    state.tick, state.last_response.handled, state.last_response.reply, state.last_response.message,
  )
}

#[cfg(feature = "guest-v2")]
fn render_v2_state(state: &ActorStateV2) -> String {
  format!(
    r#"{{"schema":2,"tick":{},"handled":{},"reply":{},"message":{:?},"upgrade_generation":{},"migrated_from_tick":{},"host_messages_seen":{},"proactive_sends":{},"last_host_sequence":{},"last_host_payload":{:?}}}"#,
    state.tick,
    state.last_response.handled,
    state.last_response.reply,
    state.last_response.message,
    state.upgrade_generation,
    state.migrated_from_tick,
    state.host_messages_seen,
    state.proactive_sends,
    state.last_host_sequence,
    state.last_host_payload,
  )
}

export!(WasmActor);
