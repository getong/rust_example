use std::{io, time::Duration};

use serde::{Deserialize, Serialize};
use tokio::{
  io::{AsyncReadExt, AsyncWriteExt},
  time::timeout,
};
use turmoil::{
  Builder, Result,
  net::{TcpListener, TcpStream},
};

const HOST_ACTOR_PORT: u16 = 7_777;

#[derive(Debug, Deserialize, Serialize)]
struct GuestMessage {
  tick: i32,
  last_handled: u64,
  last_host_reply: i32,
  payload: String,
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
struct ActorResponse {
  handled: u64,
  reply: i32,
  message: String,
}

#[test]
fn guest_process_receives_host_response_over_simulated_network() -> Result {
  let mut sim = actor_sim();

  sim.host("host", || host_actor_server(1));
  sim.client("wasm", async {
    let response = send_to_host(GuestMessage {
      tick: 6,
      last_handled: 41,
      last_host_reply: 13,
      payload: "wasm主动消息 at tick 6".to_owned(),
    })
    .await?;

    assert_eq!(
      response,
      ActorResponse {
        handled: 1,
        reply: 20,
        message: "host processed wasm主动消息 `wasm主动消息 at tick 6` after 41 handled messages"
          .to_owned(),
      }
    );

    Ok(())
  });

  sim.run()
}

#[test]
fn host_actor_preserves_handled_sequence_across_connections() -> Result {
  let mut sim = actor_sim();

  sim.host("host", || host_actor_server(3));
  sim.client("wasm", async {
    let mut responses = Vec::with_capacity(3);
    for tick in [6, 12, 18] {
      responses.push(
        send_to_host(GuestMessage {
          tick,
          last_handled: responses
            .last()
            .map_or(0, |response: &ActorResponse| response.handled),
          last_host_reply: responses
            .last()
            .map_or(0, |response: &ActorResponse| response.reply),
          payload: format!("wasm主动消息 at tick {tick}"),
        })
        .await?,
      );
    }

    assert_eq!(
      responses
        .iter()
        .map(|response| response.handled)
        .collect::<Vec<_>>(),
      vec![1, 2, 3]
    );
    assert_eq!(
      responses
        .iter()
        .map(|response| response.reply)
        .collect::<Vec<_>>(),
      vec![7, 21, 42]
    );

    Ok(())
  });

  sim.run()
}

#[test]
fn wasm_process_reconnects_after_partition_is_repaired() -> Result {
  let mut sim = actor_sim();

  sim.host("host", || host_actor_server(1));
  sim.client("wasm", async {
    turmoil::partition("wasm", "host");

    let connect_while_partitioned = TcpStream::connect(("host", HOST_ACTOR_PORT)).await;
    assert_eq!(
      connect_while_partitioned.err().map(|err| err.kind()),
      Some(io::ErrorKind::ConnectionRefused)
    );

    turmoil::repair("wasm", "host");

    let response = send_to_host(GuestMessage {
      tick: 12,
      last_handled: 0,
      last_host_reply: 0,
      payload: "wasm主动消息 at tick 12".to_owned(),
    })
    .await?;

    assert_eq!(response.handled, 1);
    assert_eq!(response.reply, 13);

    Ok(())
  });

  sim.run()
}

fn actor_sim() -> turmoil::Sim<'static> {
  Builder::new()
    .rng_seed(0xA7C0_2026)
    .tick_duration(Duration::from_millis(10))
    .simulation_duration(Duration::from_secs(5))
    .min_message_latency(Duration::from_millis(10))
    .max_message_latency(Duration::from_millis(10))
    .build()
}

async fn host_actor_server(expected_messages: u64) -> Result {
  let listener = TcpListener::bind(("0.0.0.0", HOST_ACTOR_PORT)).await?;
  let mut handled = 0;

  while handled < expected_messages {
    let (mut stream, _) = listener.accept().await?;
    let request: GuestMessage = read_json_line(&mut stream).await?;
    handled += 1;
    let response = host_response(&request, handled);

    write_json_line(&mut stream, &response).await?;
    stream.shutdown().await?;
  }

  Ok(())
}

async fn send_to_host(request: GuestMessage) -> Result<ActorResponse> {
  let mut stream = timeout(
    Duration::from_secs(1),
    TcpStream::connect(("host", HOST_ACTOR_PORT)),
  )
  .await??;

  write_json_line(&mut stream, &request).await?;
  let response = read_json_line(&mut stream).await?;
  stream.shutdown().await?;

  Ok(response)
}

async fn read_json_line<T>(stream: &mut TcpStream) -> Result<T>
where
  T: for<'de> Deserialize<'de>,
{
  let mut line = Vec::new();
  loop {
    let mut byte = [0; 1];
    let bytes = stream.read(&mut byte).await?;
    if bytes == 0 {
      return Err(
        io::Error::new(io::ErrorKind::UnexpectedEof, "peer closed before newline").into(),
      );
    }
    if byte[0] == b'\n' {
      break;
    }
    line.push(byte[0]);
  }

  Ok(serde_json::from_slice(&line)?)
}

async fn write_json_line<T>(stream: &mut TcpStream, value: &T) -> Result
where
  T: Serialize,
{
  let mut payload = serde_json::to_vec(value)?;
  payload.push(b'\n');
  stream.write_all(&payload).await?;
  Ok(())
}

fn host_response(msg: &GuestMessage, handled: u64) -> ActorResponse {
  let reply = ((msg.tick as i64 + msg.last_host_reply as i64 + handled as i64) % 997) as i32;

  ActorResponse {
    handled,
    reply,
    message: format!(
      "host processed wasm主动消息 `{}` after {} handled messages",
      msg.payload, msg.last_handled,
    ),
  }
}
