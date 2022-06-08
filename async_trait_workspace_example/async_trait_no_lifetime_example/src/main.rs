use async_trait::async_trait;
use std::fmt::Debug;

#[derive(Debug)]
struct Update;

#[async_trait]
trait Handler: Debug {
    async fn update(&self, update: &Update) -> ();
}

#[derive(Default, Debug)]
struct Dispatcher {
    handlers: Vec<Box<dyn Handler>>,
}

impl Dispatcher {
    pub fn push_handler(&mut self, handler: Box<dyn Handler>) {
        self.handlers.push(handler);
    }
}

// example handler
#[derive(Default, Debug)]
struct Foo {}
#[async_trait]
impl Handler for Foo {
    async fn update(&self, update: &Update) -> () {
        println!("Update: {:?}", update);
    }
}

#[tokio::main]
async fn main() {
    let mut dispatcher = Dispatcher::default();
    let handler = Box::new(Foo {});

    dispatcher.push_handler(handler);
    println!("dispatcher: {:?}", dispatcher);
    let result = match dispatcher.handlers.pop() {
        Some(function) => {
            let _ = function.update(&Update);
            true
        }
        _ => false,
    };
    println!("{result}");
}
