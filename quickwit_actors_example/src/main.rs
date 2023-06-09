use async_trait::async_trait;
use quickwit_actors::{Actor, ActorContext, ActorExitStatus, Handler, Mailbox, Universe};
use std::time::Duration;

#[derive(Default)]
struct PingReceiver;

impl Actor for PingReceiver {
    type ObservableState = ();
    fn observable_state(&self) -> Self::ObservableState {}
}

#[async_trait]
impl Handler<Ping> for PingReceiver {
    type Reply = String;
    async fn handle(
        &mut self,
        _msg: Ping,
        _ctx: &ActorContext<Self>,
    ) -> Result<String, ActorExitStatus> {
        Ok("Pong".to_string())
    }
}

struct PingSender {
    peer: Mailbox<PingReceiver>,
}

#[derive(Debug)]
struct Loop;

#[derive(Debug)]
struct Ping;

#[async_trait]
impl Actor for PingSender {
    type ObservableState = ();
    fn observable_state(&self) -> Self::ObservableState {}

    async fn initialize(&mut self, ctx: &ActorContext<Self>) -> Result<(), ActorExitStatus> {
        ctx.send_self_message(Loop).await?;
        Ok(())
    }
}

#[async_trait]
impl Handler<Loop> for PingSender {
    type Reply = ();

    async fn handle(&mut self, _: Loop, ctx: &ActorContext<Self>) -> Result<(), ActorExitStatus> {
        let reply_msg = ctx.ask(&self.peer, Ping).await.unwrap();
        println!("{reply_msg}");
        ctx.schedule_self_msg(Duration::from_secs(1), Loop).await;
        Ok(())
    }
}

#[tokio::main]
async fn main() {
    let universe = Universe::new();

    // let (recv_mailbox, _) = universe.spawn_actor(PingReceiver::default()).spawn();
    let (recv_mailbox, _) = universe.spawn_builder().spawn(PingReceiver);

    let ping_sender = PingSender { peer: recv_mailbox };
    // let (_, ping_sender_handler) = universe.spawn_actor(ping_sender).spawn();
    let (_, ping_sender_handler) = universe.spawn_builder().spawn(ping_sender);

    ping_sender_handler.join().await;
}
