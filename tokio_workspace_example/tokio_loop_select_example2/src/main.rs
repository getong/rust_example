// use tokio::stream::{self, StreamExt};
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::task;
// use tokio::time::{self, delay_for, timeout, Duration};
use tokio::time::{self, sleep, Duration};
use tokio_stream::StreamExt;

async fn some_computation(input: u32) -> String {
  format!("the result of computation {}", input)
}

async fn some_async_work() {
  // do work
  //delay_for(Duration::from_millis(100000)).await;
  //只需注释掉上一行代码，并追加一行无线循环代码， 即可验证select!在当前同一个task所在的thread中并发执行
  //所有<async expression>, 一旦当前thread被block住，则select!不能再check其他branch的<async expression>了
  //所以切记<async expression>中不要有block当前线程的代码！
  println!("hello world");
}

#[tokio::main]
async fn main() {
  //time::delay
  let mut delay = Box::pin(time::sleep(Duration::from_millis(5)));
  //stream
  let mut stream1 = tokio_stream::iter(vec![1, 2, 3]);
  //oneshot
  let (tx1, mut rx1) = oneshot::channel();
  tokio::spawn(async move {
    tx1.send("first").unwrap();
  });
  let mut a = None;
  //mpsc
  let (tx2, mut rx2) = mpsc::channel(100);
  tokio::spawn(async move {
    for i in 0..10 {
      let res = some_computation(i).await;
      tx2.send(res).await.unwrap();
    }
  });
  let mut done = false;
  //broadcast
  let (tx3, mut rx3) = broadcast::channel(16);
  let mut rx4 = tx3.subscribe();
  tx3.send(10).unwrap();
  tx3.send(20).unwrap();
  tokio::spawn(async move {
    assert_eq!(rx4.recv().await.unwrap(), 10);
    assert_eq!(rx4.recv().await.unwrap(), 20);
  });
  //time::interval
  let mut interval = time::interval(Duration::from_millis(2));
  //join handle
  let mut join_done = false;
  let mut join_handle: task::JoinHandle<u8> = task::spawn(async {
    // some work here
    sleep(Duration::from_millis(1)).await;
    88
  });
  //time::timeout
  //let mut to = timeout(Duration::from_millis(5), some_async_work());

  loop {
    tokio::select! {
        _ = &mut delay => {
            println!("delay reached");
            break;
        },
       /* _ = &mut to => {
            println!("operation timed out");
            break;
        },*/
        ret_code=&mut join_handle ,if !join_done => {
            join_done = true;
            println!("join handle case: {:?}", ret_code);
        },
        _= interval.tick() => {
            println!("operation interval");
        },
        _ = some_async_work() => {
            println!("operation completed");
            //delay_for(Duration::from_millis(100000)).await;
            //此处加上delay_for可用于验证， <handler code>一旦有block/long running 当前所在task的代码
            //则select!无法再去check其他branch了， 所以切记避免之！！！
        },
        Some(v) = stream1.next() => { println!("stream: {}", v);},
        v1 = (&mut rx1), if a.is_none()  =>  {
            println!("oneshot : {:?}", v1);a = v1.ok();
        },
        v2 = rx2.recv(), if !done  => {
            println!("mpsc: {:?}", v2);
             if v2.is_none() { done = true; }
        },
        v3 = rx3.recv() => {
            println!("broadcast: {:?}", v3);
        },
        else => {
            println!("not match");
        },
    }
  }
}
