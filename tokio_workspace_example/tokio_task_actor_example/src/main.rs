use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::mpsc;
// use tokio::task;

pub enum Message {
  Greet(String),
  Count(usize),
  Exit,
}

#[async_trait]
pub trait Actor: Send + Sync {
  async fn run(self: Arc<Self>, receiver: mpsc::Receiver<Message>);
  async fn send_message(&self, message: Message);
}

struct MyActor {}

#[async_trait]
impl Actor for MyActor {
  async fn run(self: Arc<Self>, mut receiver: mpsc::Receiver<Message>) {
    while let Some(message) = receiver.recv().await {
      match message {
        Message::Greet(name) => {
          println!("Hello, {}!", name);
        }
        Message::Count(n) => {
          println!("Counting to {}...", n);
          for i in 1 ..= n {
            println!("{}", i);
          }
        }
        Message::Exit => {
          println!("Exiting actor...");
          break;
        }
      }
    }
  }

  async fn send_message(&self, message: Message) {
    // Process the message
    match message {
      Message::Greet(name) => {
        println!("Hello, {}!", name);
      }
      Message::Count(n) => {
        println!("Counting to {}...", n);
        for i in 1 ..= n {
          println!("{}", i);
        }
      }
      Message::Exit => {
        println!("Exiting actor...");
      }
    }
  }
}

struct ActorHandle {
  sender: mpsc::Sender<Message>,
}

impl ActorHandle {
  async fn new<A>(actor: A) -> Self
  where
    A: 'static + Actor + Send + Sync,
  {
    let (sender, receiver) = mpsc::channel(100);
    let actor_clone = Arc::new(actor);

    tokio::spawn({
      // let sender = sender.clone();
      async move {
        actor_clone.run(receiver).await;
      }
    });

    ActorHandle { sender }
  }

  async fn send(&self, message: Message) {
    let _ = self.sender.send(message).await;
  }
}

#[tokio::main]
async fn main() {
  let actor = MyActor {};

  let actor_handle = ActorHandle::new(actor).await;

  // Send some messages to the actor
  actor_handle.send(Message::Greet("Alice".to_owned())).await;
  actor_handle.send(Message::Count(5)).await;
  actor_handle.send(Message::Greet("Bob".to_owned())).await;
  actor_handle.send(Message::Exit).await;

  // Wait for the actor task to complete
  tokio::time::sleep(std::time::Duration::from_secs(1)).await;
}
