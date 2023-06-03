use futures::future::FutureExt;
use std::time::Duration;
use tokio::{sync::oneshot, time};
use tokio_stream::{self as stream, StreamExt};

#[tokio::main]
async fn main() {
    let mut stream1 = stream::iter(vec![1, 2, 3]);

    let (stop_read, time_to_stop): (oneshot::Sender<()>, _) = oneshot::channel();
    tokio::spawn(async move {
        time::sleep(Duration::from_millis(3000)).await;
        if let Err(_) = stop_read.send(()) {
            eprintln!("somthing goes wrong");
        }
    });

    let (tx1, rx1) = oneshot::channel();
    let (tx2, rx2) = oneshot::channel();

    tokio::spawn(async {
        let _ = tx1.send("one");
    });

    tokio::spawn(async {
        let _ = tx2.send("two");
    });

    let mut time_to_stop = time_to_stop.fuse();
    let mut rx1 = rx1.fuse();
    let mut rx2 = rx2.fuse();

    loop {
        let next = tokio::select! {
            Some(v) = stream1.next() => {
                time::sleep(Duration::from_millis(50)).await;
                v
            }

            _ = &mut time_to_stop => {
                println!("time_to_stop trigger");
                return;
            }

            Ok(val) = &mut rx1 => {
                println!("rx1 completed first with {:?}", val);
                4
            }

            Ok(val) = &mut rx2 => {
                println!("rx2 completed first with {:?}", val);
                5
            }

            else => {
                println!("else" );
                6
            }
        };

        println!("next: {}", next);
    }
}
