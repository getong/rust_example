use std::{thread, time::Duration};

use futures::{
  future::ready,
  // select,
  stream::{FusedStream, FuturesUnordered, Stream},
  SinkExt,
  StreamExt,
};
use tokio::runtime;

struct G;

impl G {
  async fn ref_foo(&self) {
    println!("ref_foo +++");
    tokio::time::sleep(Duration::from_millis(500)).await;
    println!("ref_foo ---");
  }
  async fn mut_foo(&mut self) {
    println!("mut_foo +++");
    tokio::time::sleep(Duration::from_millis(500)).await;
    println!("mut_foo ---");
  }
}

#[derive(Clone)]
enum Task {
  TR,
  TM,
}

// wrappers
async fn run_ref_task(g: &G, task: Task) {
  match task {
    Task::TR => g.ref_foo().await,
    _ => {}
  };
}

async fn run_mut_task(mut g: G, task: Task) -> G {
  match task {
    Task::TM => g.mut_foo().await,
    _ => {}
  };
  g
}

async fn run_loop(mut rx: impl Stream<Item = Task> + FusedStream + Unpin) {
  let g0 = G;

  let mut getter = FuturesUnordered::new();
  getter.push(ready(g0));
  // the following streams stores only `ready(task)`
  let mut mut_tasks = FuturesUnordered::new(); // for tasks that's scheduled in this loop
  let mut ref_tasks = FuturesUnordered::new();
  let mut mut_delay = FuturesUnordered::new(); // for tasks that's scheduled in next loop
  let mut ref_delay = FuturesUnordered::new();

  loop {
    println!("============ avoid idle loops ============");
    let g = getter.select_next_some().await;
    {
      let mut queries = FuturesUnordered::new(); // where we schedule ref_foo tasks
      loop {
        println!("------------ avoid idle ref_task loops ------------");
        tokio::select! {
            task = rx.select_next_some() => {
                match &task {
                    Task::TR => ref_delay.push(ready(task)),
                    Task::TM => mut_tasks.push(ready(task)),
                };
                if mut_delay.is_empty() && ref_tasks.is_empty() && queries.is_empty() { break; }
            },
            task = mut_delay.select_next_some() => {
                mut_tasks.push(ready(task));
                if mut_delay.is_empty() && ref_tasks.is_empty() && queries.is_empty() { break; }
            }
            task = ref_tasks.select_next_some() => {
                queries.push(run_ref_task(&g, task));
            }
            _ = queries.select_next_some() => {
                if mut_delay.is_empty() && ref_tasks.is_empty() && queries.is_empty() { break; }
            },
        }
      }
    }
    getter.push(ready(g));

    {
      let mut queries = FuturesUnordered::new(); // where we schedule mut_foo tasks
      loop {
        println!("------------ avoid idle mut_task loops ------------");
        tokio::select! {
            task = rx.select_next_some() => {
                match &task {
                    Task::TR => ref_tasks.push(ready(task)),
                    Task::TM => mut_delay.push(ready(task)),
                };
                if ref_delay.is_empty() && mut_tasks.is_empty() && queries.is_empty() { break; }
            },
            task = ref_delay.select_next_some() => {
                ref_tasks.push(ready(task));
                if ref_delay.is_empty() && mut_tasks.is_empty() && queries.is_empty() { break; }
            }
            g = getter.select_next_some() => {
                if let Some(task) = mut_tasks.next().await {
                    queries.push(run_mut_task(g, task));
                } else {
                    getter.push(ready(g));
                    if ref_delay.is_empty() && queries.is_empty() { break; }
                }
            }
            g = queries.select_next_some() => {
                getter.push(ready(g));
                if ref_delay.is_empty() && mut_tasks.is_empty() && queries.is_empty() { break; }
            }
        }
      }
    }
  }
}

fn main() {
  let (mut tx, rx) = futures::channel::mpsc::channel(10000);
  let th = thread::spawn(move || thread_main(rx));
  let tasks = vec![
    Task::TR,
    Task::TR,
    Task::TM,
    Task::TM,
    Task::TR,
    Task::TR,
    Task::TR,
    Task::TM,
    Task::TM,
  ];

  let rt = runtime::Builder::new_multi_thread()
    .enable_time()
    .build()
    .unwrap();
  rt.block_on(async {
    loop {
      for task in tasks.clone() {
        tx.send(task).await.expect("");
      }
      tokio::time::sleep(Duration::from_secs(10)).await;
    }
  });
  th.join().expect("");
}

fn thread_main(rx: futures::channel::mpsc::Receiver<Task>) {
  let rt = runtime::Builder::new_multi_thread()
    .enable_time()
    .build()
    .unwrap();
  rt.block_on(async {
    run_loop(rx).await;
  });
}
