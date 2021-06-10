use actix::prelude::*;
use std::thread;

struct Ping(usize);

impl Message for Ping {
    type Result = usize;
}

struct MyActor {
    count: usize,
}

impl Actor for MyActor {
    type Context = Context<Self>;
}

impl Handler<Ping> for MyActor {
    type Result = usize;

    fn handle(&mut self, msg: Ping, _: &mut Context<Self>) -> Self::Result {
        println!("Handler in {:?}", thread::current().id());
        self.count += msg.0;
        self.count
    }
}

fn main() {
    // println!("Hello, world!");
    let system = System::new();

    Arbiter::new().spawn(async {
        // 启动一个 actor
        let addr = MyActor { count: 10 }.start();

        println!("Arbiter in {:?}", thread::current().id());

        // 发送数据
        let res = addr.send(Ping(10)).await;

        println!("RESULT: {}", res.unwrap() == 20);

        // 停止系统退出
        System::current().stop();
    });

    system.run().unwrap();
}
