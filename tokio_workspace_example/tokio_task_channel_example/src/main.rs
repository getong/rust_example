use tokio::sync::mpsc::{self, Receiver, Sender};

fn main() {
  let n = std::env::args_os()
    .nth(1)
    .and_then(|s| s.into_string().ok())
    .and_then(|s| s.parse().ok())
    .unwrap_or(100);

  async_main(n).unwrap();
}

#[tokio::main]
async fn async_main(n: usize) -> anyhow::Result<(), anyhow::Error> {
  let (sender, mut receiver) = mpsc::channel::<usize>(2);
  tokio::spawn(generate(sender));
  for _i in 0 .. n {
    let prime = receiver.recv().await.unwrap();
    println!("{}", prime);
    let (sender_next, receiver_next) = mpsc::channel::<usize>(2);
    tokio::spawn(filter(receiver, sender_next, prime));
    receiver = receiver_next;
  }
  Ok(())
}

async fn generate(sender: Sender<usize>) -> anyhow::Result<(), anyhow::Error> {
  let mut i = 2;
  while sender.send(i).await.is_ok() {
    i += 1;
  }
  Ok(())
}

async fn filter(
  mut receiver: Receiver<usize>,
  sender: Sender<usize>,
  prime: usize,
) -> anyhow::Result<(), anyhow::Error> {
  while let Some(i) = receiver.recv().await {
    if i % prime != 0 {
      if sender.send(i).await.is_err() {
        return Ok(());
      }
    }
  }
  Ok(())
}
