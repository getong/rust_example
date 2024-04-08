use async_trait::async_trait;
use pptr::prelude::*;

#[derive(Default, Clone)]
struct PingActor;

#[async_trait]
impl Lifecycle for PingActor {
  type Supervision = OneForAll;
}

#[derive(Debug)]
struct Ping(u32);

#[async_trait]
impl Handler<Ping> for PingActor {
  type Response = ();
  type Executor = SequentialExecutor;

  async fn handle_message(
    &mut self,
    msg: Ping,
    ctx: &Context,
  ) -> Result<Self::Response, PuppetError> {
    println!("Ping received: {}", msg.0);
    if msg.0 < 10 {
      ctx.send::<PongActor, _>(Pong(msg.0 + 1)).await?;
    } else {
      println!("Ping-Pong finished!");
    }
    Ok(())
  }
}

#[derive(Clone, Default)]
struct PongActor;

#[async_trait]
impl Lifecycle for PongActor {
  type Supervision = OneForAll;
}

#[derive(Debug)]
struct Pong(u32);

#[async_trait]
impl Handler<Pong> for PongActor {
  type Response = ();
  type Executor = SequentialExecutor;

  async fn handle_message(
    &mut self,
    msg: Pong,
    ctx: &Context,
  ) -> Result<Self::Response, PuppetError> {
    println!("Pong received: {}", msg.0);
    if msg.0 < 10 {
      ctx.send::<PingActor, _>(Ping(msg.0 + 1)).await?;
    } else {
      println!("Ping-Pong finished!");
    }
    Ok(())
  }
}

#[tokio::main]
async fn main() -> Result<(), PuppetError> {
  let pptr = Puppeter::new();

  pptr.puppet_builder(PingActor::default()).spawn().await?;
  pptr.puppet_builder(PongActor::default()).spawn().await?;

  pptr.send::<PingActor, _>(Ping(0)).await?;

  tokio::time::sleep(std::time::Duration::from_secs(1)).await;
  Ok(())
}
