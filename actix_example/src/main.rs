use actix::prelude::*;
use std::time::Duration;

#[derive(Message)]
#[rtype(result = "()")]
struct Ping {
    pub id: usize,
}

// Actor definition
struct Game {
    counter: usize,
    name: String,
    recipient: Recipient<Ping>,
}

impl Actor for Game {
    type Context = Context<Game>;
}

// simple message handler for Ping message
impl Handler<Ping> for Game {
    type Result = ();

    fn handle(&mut self, msg: Ping, ctx: &mut Context<Self>) {
        self.counter += 1;

        if self.counter > 10 {
            System::current().stop();
        } else {
            println!("[{0}] Ping received {1}", self.name, msg.id);

            // wait 100 nanoseconds
            ctx.run_later(Duration::new(0, 100), move |act, _| {
                act.recipient.do_send(Ping { id: msg.id + 1 }).unwrap();
            });
        }
    }
}

fn main() {
    let system = System::new();

    // To get a Recipient object, we need to use a different builder method
    // which will allow postponing actor creation
    _ = system.block_on(async {
        Game::create(|ctx| {
            // now we can get an address of the first actor and create the second actor
            let addr = ctx.address();

            let addr2 = Game {
                counter: 0,
                name: String::from("Game 2"),
                recipient: addr.recipient(),
            }
            .start();

            // let's start pings
            addr2.do_send(Ping { id: 10 });

            // now we can finally create first actor
            Game {
                counter: 0,
                name: String::from("Game 1"),
                recipient: addr2.recipient(),
            }
        });
    });

    system.run().unwrap();
}
