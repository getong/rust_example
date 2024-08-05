use futures::future::FutureExt;
use tokio::sync::oneshot;

#[tokio::main]
async fn main() {
  let (tx1, rx1) = oneshot::channel();
  let (tx2, rx2) = oneshot::channel();

  tokio::spawn(async move {
    let _ = tx1.send("one");
  });

  tokio::spawn(async move {
    let _ = tx2.send("two");
  });

  let mut rx1 = rx1.fuse();
  let mut rx2 = rx2.fuse();

  for _ in 0 .. 2 {
    tokio::select! {
        val = &mut rx1 => {
            println!("rx1 comopleted first with {:?}", val);
        }
        val = &mut rx2 => {
            println!("rx2 comopleted first with {:?}", val);
        }
    }
  }
}
