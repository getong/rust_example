#![cfg(target_arch = "wasm32")]

wit_bindgen::generate!({
  path: "src/wit",
  world: "actor-world",
});

// wit_bindgen generates demo::actor::host_actor::{GuestMessage, HostMessage, ActorResponse}
// from actor.wit.  The WASM guest works with these typed structs directly — no JSON, no
// protocol.rs, no serde on the hot path.
use crate::demo::actor::host_actor;
use host_actor::{ActorResponse, GuestMessage};

const LOOP_SLEEP_MILLIS: i32 = 500;
const WASM_TO_HOST_INTERVAL_MILLIS: i32 = 3_000;

struct WasmActor;

impl exports::wasm_actor::Guest for WasmActor {
  fn run_loop(max_ticks: i32) -> String {
    println!("wasm actor: entering resident loop; max_ticks={max_ticks}");

    let mut tick = 0i32;
    let mut last_host_reply = 0i32;
    let mut elapsed_since_push = 0i32;
    // Default response used before the first send_to_host call.
    let mut last_response = ActorResponse {
      handled: 0,
      reply: 0,
      message: String::new(),
    };

    loop {
      tick += 1;
      elapsed_since_push += LOOP_SLEEP_MILLIS;

      // recv_from_host() → Option<host_actor::HostMessage> — typed, no JSON decode.
      if let Some(msg) = host_actor::recv_from_host() {
        println!(
          "wasm actor: received host主动消息 #{} payload={}",
          msg.sequence, msg.payload
        );
      }

      if elapsed_since_push >= WASM_TO_HOST_INTERVAL_MILLIS {
        elapsed_since_push = 0;

        // Construct a typed GuestMessage — fields come directly from StoreState's
        // channel definition in host.rs.  No serde_json::to_string needed.
        let request = GuestMessage {
          tick,
          last_host_reply,
          payload: format!("wasm主动消息 at tick {tick}"),
        };
        println!(
          "wasm actor: actively sending message to host: tick={tick} payload={}",
          request.payload
        );

        // send_to_host(GuestMessage) → ActorResponse — typed round-trip through
        // StoreState.host_actor_tx → host thread → StoreState.host_actor_tx reply.
        last_response = host_actor::send_to_host(&request);
        println!(
          "wasm actor: host reply handled #{} reply={} message={}",
          last_response.handled, last_response.reply, last_response.message
        );
        last_host_reply = last_response.reply;
      }

      if max_ticks > 0 && tick >= max_ticks {
        println!("wasm actor: leaving verification loop after {tick} ticks");
        // Serialize final response to a JSON-like string for the host's println.
        break format!(
          r#"{{"handled":{},"reply":{},"message":{:?}}}"#,
          last_response.handled, last_response.reply, last_response.message,
        );
      }

      host_actor::sleep_millis(LOOP_SLEEP_MILLIS);
    }
  }
}

export!(WasmActor);
