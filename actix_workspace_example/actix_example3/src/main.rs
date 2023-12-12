use actix::prelude::*;

struct MyActor;
impl Actor for MyActor {
  type Context = Context<Self>;
}

struct Ping;
impl Message for Ping {
  type Result = ();
}

impl Handler<Ping> for MyActor {
  type Result = ();
  fn handle(&mut self, _: Ping, ctx: &mut Context<Self>) {
    println!("Ping received!");
    ctx.stop();
  }
}

#[actix::main]
async fn main() {
  let my_actor = MyActor.start();

  _ = my_actor.send(Ping).await;

  System::current().stop();
}
