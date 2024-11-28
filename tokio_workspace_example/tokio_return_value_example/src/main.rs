use tokio::{
  sync::mpsc,
  time::{sleep, Duration},
};

async fn foo() -> i32 {
  sleep(Duration::from_secs(2)).await;
  10
}

async fn bar() -> i32 {
  sleep(Duration::from_secs(3)).await;
  20
}

async fn foo_send(tx: mpsc::Sender<i32>) {
  loop {
    sleep(Duration::from_secs(2)).await;
    tx.send(10).await.unwrap();
  }
}

async fn bar_send(tx: mpsc::Sender<i32>) {
  loop {
    sleep(Duration::from_secs(3)).await;
    tx.send(20).await.unwrap();
  }
}

#[tokio::main]
async fn main() {
  let result = tokio::select! {
      result = foo() => result,
      result = bar() => result,
  };

  println!("Result: {}", result);

  let (tx1, mut rx1) = mpsc::channel::<i32>(128);
  let (tx2, mut rx2) = mpsc::channel::<i32>(128);

  tokio::spawn(foo_send(tx1));
  tokio::spawn(bar_send(tx2));
  loop {
    let result = tokio::select! {
        // result = rx1.recv() => result.unwrap(),
        // result = rx2.recv() => result.unwrap(),
        Some(msg) = rx1.recv() => msg,
        Some(msg) = rx2.recv() => msg,
        else => {
            println!("the else ");
            break }
    };

    println!("recv Result: {}", result);
  }

  // send and recv can not be in the same thread
  // let (tx1, mut rx1) = mpsc::channel(128);
  // let (tx2, mut rx2) = mpsc::channel(128);
  // let (tx3, mut rx3) = mpsc::channel(128);
  // _ = tx1.send("a");
  // _ = tx2.send("b");
  // _ = tx3.send("c");

  // loop {
  //     let msg = tokio::select! {
  //         msg = rx1.recv() => msg.unwrap(),
  //         msg = rx2.recv() => msg.unwrap(),
  //         msg = rx3.recv() => msg.unwrap(),
  //         else => {
  //             println!("the else ");
  //             break }
  //     };

  //     println!("Got {}", msg);
  // }
}
