use tokio::{
  runtime::Builder,
  sync::{mpsc, oneshot},
  task::LocalSet,
};

// This struct describes the task you want to spawn. Here we include
// some simple examples. The oneshot channel allows sending a response
// to the spawner.
#[derive(Debug)]
pub enum Task {
  PrintNumber(u32),
  AddOne(u32, oneshot::Sender<u32>),
}

#[derive(Clone)]
struct LocalSpawner {
  send: mpsc::UnboundedSender<Task>,
}

impl LocalSpawner {
  pub fn new() -> Self {
    let (send, mut recv) = mpsc::unbounded_channel();

    let rt = Builder::new_current_thread().enable_all().build().unwrap();

    std::thread::spawn(move || {
      let local = LocalSet::new();

      local.spawn_local(async move {
        while let Some(new_task) = recv.recv().await {
          tokio::task::spawn_local(run_task(new_task));
        }
        // If the while loop returns, then all the LocalSpawner
        // objects have been dropped.
      });

      // This will return once all senders are dropped and all
      // spawned tasks have returned.
      rt.block_on(local);
    });

    Self { send }
  }

  pub fn spawn(&self, task: Task) {
    self
      .send
      .send(task)
      .expect("Thread with LocalSet has shut down.");
  }
}

// This task may do !Send stuff. We use printing a number as an example,
// but it could be anything.
//
// The Task struct is an enum to support spawning many different kinds
// of operations.
async fn run_task(task: Task) {
  match task {
    Task::PrintNumber(n) => {
      println!("{}", n);
    }
    Task::AddOne(n, response) => {
      // We ignore failures to send the response.
      let _ = response.send(n + 1);
    }
  }
}

#[tokio::main]
async fn main() {
  let spawner = LocalSpawner::new();

  let (send, response) = oneshot::channel();
  spawner.spawn(Task::AddOne(10, send));
  let eleven = response.await.unwrap();
  assert_eq!(eleven, 11);
}
