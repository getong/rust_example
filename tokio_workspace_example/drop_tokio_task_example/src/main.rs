// copy from [Rust Async Drop](https://stackoverflow.com/questions/71541765/rust-async-drop)
use std::time::Duration;

use tokio::{
  // runtime::Runtime,
  // sync::oneshot::{self, Receiver, Sender},
  sync::oneshot::{self, Sender},
  time::interval,
};

struct MyClass {
  tx: Option<Sender<()>>, /* can have SomeStruct instead of ()
                           * my_state: Option<SomeStruct> */
}

impl MyClass {
  pub async fn new() -> Self {
    println!("MyClass::new()");

    let (tx, mut rx) = oneshot::channel();

    tokio::task::spawn(async move {
      let mut interval = interval(Duration::from_millis(100));

      println!("drop wait loop starting...");

      loop {
        tokio::select! {
            _ = interval.tick() => println!("Another 100ms"),
            _msg = &mut rx => {
                println!("should process drop here");
                break;
            }
        }
      }
    });

    Self { tx: Some(tx) }
  }
}

impl Drop for MyClass {
  fn drop(&mut self) {
    println!("drop()");
    self.tx.take().unwrap().send(()).unwrap();
    // self.tx.take().unwrap().send(self.my_state.take().unwrap()).unwrap();
  }
}

#[tokio::main]
async fn main() {
  // let class = MyClass::new().await;
  // drop(class);
  let _class = MyClass::new().await;
}
