use std::time::Duration;

use kameo::{error::Infallible, prelude::*};

// cargo install kameo_console
// kameo-console
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let console = kameo::console::serve("127.0.0.1:9999").await?;
  let _traffic = TrafficRoot::spawn(());

  println!(
    "serving console on {} - connect with `kameo-console` cli",
    console.local_addr()
  );

  tokio::signal::ctrl_c().await?;
  Ok(())
}

struct TrafficRoot {
  _ticker: ActorRef<Ticker>,
  _slow_worker: ActorRef<SlowWorker>,
}

impl Actor for TrafficRoot {
  type Args = ();
  type Error = Infallible;

  fn name() -> &'static str {
    "traffic-root"
  }

  async fn on_start(_: (), root: ActorRef<Self>) -> Result<Self, Self::Error> {
    let ticker = Ticker::supervise(&root, Ticker::default()).spawn().await;
    let slow_worker = SlowWorker::supervise(&root, SlowWorker::default())
      .spawn_with_mailbox(mailbox::bounded(8))
      .await;

    drive_ticker(ticker.clone());
    drive_slow_worker(slow_worker.clone());

    Ok(TrafficRoot {
      _ticker: ticker,
      _slow_worker: slow_worker,
    })
  }
}

#[derive(Clone, Default)]
struct Ticker {
  ticks: u64,
}

impl Actor for Ticker {
  type Args = Self;
  type Error = Infallible;

  fn name() -> &'static str {
    "ticker"
  }

  async fn on_start(state: Self::Args, _: ActorRef<Self>) -> Result<Self, Self::Error> {
    Ok(state)
  }
}

struct Tick;

impl Message<Tick> for Ticker {
  type Reply = u64;

  async fn handle(&mut self, _: Tick, _: &mut Context<Self, Self::Reply>) -> Self::Reply {
    self.ticks += 1;
    self.ticks
  }
}

#[derive(Clone, Default)]
struct SlowWorker {
  jobs: u64,
}

impl Actor for SlowWorker {
  type Args = Self;
  type Error = Infallible;

  fn name() -> &'static str {
    "slow-worker"
  }

  async fn on_start(state: Self::Args, _: ActorRef<Self>) -> Result<Self, Self::Error> {
    Ok(state)
  }
}

struct Work;

impl Message<Work> for SlowWorker {
  type Reply = u64;

  async fn handle(&mut self, _: Work, _: &mut Context<Self, Self::Reply>) -> Self::Reply {
    self.jobs += 1;
    tokio::time::sleep(Duration::from_millis(700)).await;
    self.jobs
  }
}

fn drive_ticker(ticker: ActorRef<Ticker>) {
  tokio::spawn(async move {
    let mut interval = tokio::time::interval(Duration::from_millis(250));
    loop {
      interval.tick().await;
      if ticker.tell(Tick).await.is_err() {
        break;
      }
    }
  });
}

fn drive_slow_worker(slow_worker: ActorRef<SlowWorker>) {
  tokio::spawn(async move {
    let mut interval = tokio::time::interval(Duration::from_millis(120));
    loop {
      interval.tick().await;
      if slow_worker.tell(Work).await.is_err() {
        break;
      }
    }
  });
}
