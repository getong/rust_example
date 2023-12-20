use futures::FutureExt;
use futures::StreamExt;
use futures::{
  channel::oneshot,
  stream::Stream,
  task::{Context, Poll},
};
use std::pin::Pin;
// use std::sync::Arc;

#[derive(Debug)]
pub enum Cmd {
  Command1,
  Command2,
  Command3,
}

#[derive(Debug)]
pub struct CtlHandler {
  pub cmd: Cmd,
}

impl CtlHandler {
  fn new(cmd: Cmd) -> Self {
    CtlHandler { cmd }
  }

  async fn process(self) {
    // Simulate asynchronous processing based on the command
    println!("Processing command: {:?}", self.cmd);
  }
}

pub struct CtlAcceptor {
  pub shutdown_trigger: oneshot::Receiver<()>,
  pub command_stream: tokio::sync::mpsc::Receiver<Cmd>, // Using Tokio mpsc for simplicity
}

impl CtlAcceptor {
  pub fn new(
    shutdown_trigger: oneshot::Receiver<()>,
    command_stream: tokio::sync::mpsc::Receiver<Cmd>,
  ) -> Self {
    CtlAcceptor {
      shutdown_trigger,
      command_stream,
    }
  }
}

impl Stream for CtlAcceptor {
  type Item = CtlHandler;

  fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
    match self.shutdown_trigger.poll_unpin(cx) {
      Poll::Ready(Ok(())) => {
        println!("Signal received; stopping CtlAcceptor");
        Poll::Ready(None)
      }
      Poll::Ready(Err(e)) => {
        println!("Error polling CtlAcceptor shutdown trigger: {}", e);
        Poll::Ready(None)
      }
      Poll::Pending => match self.command_stream.poll_recv(cx) {
        Poll::Ready(Some(cmd)) => {
          let task = CtlHandler::new(cmd);
          Poll::Ready(Some(task))
        }
        Poll::Ready(None) => {
          println!("Command stream closed; stopping CtlAcceptor");
          Poll::Ready(None)
        }
        Poll::Pending => Poll::Pending,
      },
    }
  }
}

#[tokio::main]
async fn main() {
  // Create a channel for commands
  let (cmd_sender, cmd_receiver) = tokio::sync::mpsc::channel(16);

  // Create a channel for shutdown signal
  let (ctl_shutdown, shutdown_trigger) = oneshot::channel();

  // Create an instance of CtlAcceptor
  let ctl_acceptor = CtlAcceptor::new(shutdown_trigger, cmd_receiver);

  // Spawn a Tokio task to simulate sending commands
  tokio::spawn(async move {
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    cmd_sender.send(Cmd::Command1).await.unwrap();
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    cmd_sender.send(Cmd::Command2).await.unwrap();
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    cmd_sender.send(Cmd::Command3).await.unwrap();
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    drop(cmd_sender);
  });

  // Process commands using the CtlAcceptor stream
  let mut ctl_stream = Box::pin(ctl_acceptor);
  while let Some(ctl_handler) = ctl_stream.next().await {
    tokio::spawn(ctl_handler.process());
  }
  ctl_shutdown.send(()).ok();
}
