// use futures::future::FutureExt;
use std::time::{SystemTime, UNIX_EPOCH};
//use tokio::sync::oneshot;
// use rand::distributions::{Distribution, Uniform};
// use rand::{thread_rng, Rng};
use rand::Rng;
// use std::rc::Rc;
//use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::{self, sleep, Duration};

async fn random_number() -> u64 {
    let mut rng = rand::thread_rng();
    rng.gen_range(1u64..3u64)
}

#[tokio::main]
async fn main() {
    // println!("Hello, world!");
    let mut interval = time::interval(Duration::from_secs(2));

    // let start = SystemTime::now();
    //let since_the_epoch = start
    //    .duration_since(UNIX_EPOCH)
    //.expect("Time went backwards");

    // let (tx1, mut rx1) = oneshot::channel::<u8>();
    let (tx1, mut rx1) = mpsc::channel(32);

    tokio::spawn(async move {
        let mut i = 0;

        loop {
            let rand_sec = random_number().await;
            let _ = sleep(Duration::from_secs(rand_sec)).await;
            tx1.send(i).await.unwrap();
            i += 1;
        }
    });

    loop {
        tokio::select! {
            val = interval.tick() => {
                let start = SystemTime::now();
                let since_the_epoch = start
                    .duration_since(UNIX_EPOCH)
                    .expect("Time went backwards");
                println!("operation interval, current time: {}, value: {:?}", since_the_epoch.as_secs(), val);
            },

            Some(val) = rx1.recv() => {
                let start = SystemTime::now();
                let since_the_epoch = start
                    .duration_since(UNIX_EPOCH)
                    .expect("Time went backwards");
                println!("recv from rx1, val: {}, current time: {}", val, since_the_epoch.as_secs());
                drop(interval);
                interval = time::interval(Duration::from_secs(4));
            },

            else => {
                    println!("other info ");
                },

        }
    }
}
