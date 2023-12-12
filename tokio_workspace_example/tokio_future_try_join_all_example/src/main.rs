use rand::Rng;

struct Work {
  request: String,
}

struct Result {
  response: String,
}

async fn do_work(work: Work) -> Result {
  let rng = rand::thread_rng().gen_range(500..1500);
  tokio::time::sleep(std::time::Duration::from_millis(rng)).await;

  Result {
    response: format!("{}_processed", work.request),
  }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  let worker_count = 5;

  let (tx_work, rx_work): (async_channel::Sender<Work>, async_channel::Receiver<Work>) =
    async_channel::bounded(1);

  let (tx_result, rx_result): (
    async_channel::Sender<Result>,
    async_channel::Receiver<Result>,
  ) = async_channel::bounded(1);

  let mut workers = Vec::new();

  for _ in 0..worker_count {
    workers.push(tokio::spawn(worker(rx_work.clone(), tx_result.clone())));
  }

  let consumer = tokio::spawn(consumer(rx_result));

  for idx in 0..20 {
    tx_work
      .send(Work {
        request: format!("work_{}", idx),
      })
      .await?;
  }
  // Indicate that no more work will be sent by closing the channel. This will allow the worker loops to complete.
  drop(tx_work);

  // Wait for all workers to be done.
  futures::future::try_join_all(workers).await?;

  // Close the tx_result channel since workers will not send on that anymore. All of the workers will have exited at this point and dropped their clones of this, so dropping this last sender closes the channel.
  drop(tx_result);

  // Wait for the consumer to be done. All senders to the result channel are closed which will allow the consumer loop to end.
  consumer.await?;

  Ok(())
}

async fn worker(input: async_channel::Receiver<Work>, output: async_channel::Sender<Result>) {
  loop {
    match input.recv().await {
      Ok(work) => {
        if output.send(do_work(work).await).await.is_err() {
          return;
        };
      }
      Err(e) => {
        println!("shutting down worker: {}", e);
        return;
      }
    }
  }
}

async fn consumer(input: async_channel::Receiver<Result>) {
  loop {
    match input.recv().await {
      Ok(result) => println!("{}", result.response),
      Err(e) => {
        println!("shutting down consumer: {}", e);
        return;
      }
    }
  }
}

// copy from https://willbaker.dev/posts/rust-networking-concurrency/
