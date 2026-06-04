#![cfg(target_arch = "wasm32")]

wit_bindgen::generate!({
  path: "src/wit",
  world: "actor-world",
});

use host_actor::{ActorMsg, ActorMsgKind, ActorState, GuestMessage};

use crate::demo::actor::host_actor;

const LOOP_SLEEP_MILLIS: i32 = 500;
const WASM_TO_HOST_INTERVAL_MILLIS: i32 = 3_000;

struct WasmActor;

impl exports::wasm_actor::Guest for WasmActor {
  fn handle_call(msgs: Vec<ActorMsg>, mut state: ActorState) -> ActorState {
    for msg in msgs {
      state = match msg.kind {
        ActorMsgKind::Tick => {
          state.tick += 1;
          state.elapsed_since_push += LOOP_SLEEP_MILLIS;
          if state.elapsed_since_push >= WASM_TO_HOST_INTERVAL_MILLIS {
            state.elapsed_since_push = 0;

            let request = GuestMessage {
              tick: state.tick,
              last_handled: state.last_response.handled,
              last_host_reply: state.last_host_reply,
              payload: format!("wasm主动消息 at tick {}", state.tick),
            };
            println!(
              "wasm actor: actively sending message to host: tick={} payload={}",
              request.tick, request.payload
            );

            let response = host_actor::send_to_host(&request);
            println!(
              "wasm actor: host reply handled #{} reply={} message={}",
              response.handled, response.reply, response.message
            );
            state.last_host_reply = response.reply;
            state.last_response = response;
          }
          state
        }
        ActorMsgKind::HostMessage => {
          if let Some(msg) = msg.host_message {
            println!(
              "wasm actor: received host主动消息 #{} payload={}",
              msg.sequence, msg.payload
            );
          }
          state
        }
      };
    }
    state
  }

  fn render_state(state: ActorState) -> String {
    format!(
      r#"{{"tick":{},"handled":{},"reply":{},"message":{:?}}}"#,
      state.tick,
      state.last_response.handled,
      state.last_response.reply,
      state.last_response.message,
    )
  }
}

export!(WasmActor);
