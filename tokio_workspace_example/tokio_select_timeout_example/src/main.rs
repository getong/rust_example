use std::{io::stdin, thread, time::Duration};

use tokio::{sync::mpsc, time::sleep};

async fn create_timeout(duration: Duration) {
  tokio::spawn(sleep(duration)).await.unwrap();
}

async fn get_user_input() -> String {
  let (s, mut r) = mpsc::unbounded_channel();
  thread::spawn(move || {
    let mut input = String::new();
    stdin().read_line(&mut input).unwrap();
    s.send(input).unwrap();
  });
  r.recv().await.unwrap()
}

#[tokio::main]
async fn main() {
  let timeout = create_timeout(Duration::from_secs(2));
  let user_input = get_user_input();

  let interval_timer = sleep(Duration::from_millis(300));
  let mut interval_timer = std::pin::pin!(interval_timer);

  tokio::select! {
      input = user_input => println!("User entered: {:?}", input),
      _ = timeout => println!("Timed out"),
      _ = interval_timer.as_mut() => {
          println!("300 millisecond");
      }
  };

  let handler = tokio::spawn(async move {
    loop {
      let timer = sleep(Duration::from_millis(300));
      let mut timer = std::pin::pin!(timer);

      tokio::select! {
          _ = timer.as_mut() => {
              println!("300 millisecond in the task");
          }
      }
    }
  });
  _ = handler.await;
}
