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
fn main() {
    let system = System::new();
    let my_actor = MyActor.start();
    my_actor.do_send(Ping);
    system.run().unwrap();
}