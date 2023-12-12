use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

pub struct Tasker {
  queue: VecDeque<Box<dyn Fn() + Send>>,
  num_threads: u8,
  pool: ThreadPool,
}

impl Tasker {
  pub fn new(size: u8) -> Self {
    Tasker {
      queue: VecDeque::new(),
      num_threads: size,
      pool: ThreadPool::new(size),
    }
  }

  pub fn add<F: Fn() + Send + 'static>(&mut self, f: F) {
    self.queue.push_front(Box::new(f));
  }

  pub fn execute(self) {
    self.pool.execute(self.queue);
  }

  pub fn get_num_threads(&self) -> u8 {
    self.num_threads
  }
}

pub struct ThreadPool {
  size: u8,
  tasks: Arc<Mutex<VecDeque<Box<dyn Fn() + Send>>>>,
  handles: Vec<std::thread::JoinHandle<()>>,
}

impl ThreadPool {
  fn new(size: u8) -> Self {
    let tasks = Arc::new(Mutex::new(VecDeque::new()));
    let started = Arc::new(Mutex::new(false));
    let mut handles = Vec::new();

    for _ in 0..size {
      let task_queue = tasks.clone();
      let start_tracker = started.clone();

      handles.push(std::thread::spawn(move || loop {
        let mut m_guard = task_queue.lock().unwrap();
        let mut start_guard = start_tracker.lock().unwrap();
        //if queue is empty and no thread has started, then simply continue running
        //if queue is empty and a thread has started, it means that the entire queue is processed and it is time to stop
        if (*m_guard).is_empty() && *start_guard {
          return;
        }
        //if queue contains tasks, each available thread will pull one task out of the queue and process it
        if !(*m_guard).is_empty() {
          let task: Box<dyn Fn() + Send> = (*m_guard).pop_back().unwrap();
          *start_guard = true;
          drop(m_guard);
          drop(start_guard);
          task();
        }
      }));
    }

    ThreadPool {
      size,
      tasks,
      handles,
    }
  }

  fn execute(self, tasks: VecDeque<Box<dyn Fn() + Send>>) {
    let mut task_queue = self.tasks.lock().unwrap();
    *task_queue = tasks;
    drop(task_queue);
    for handle in self.handles {
      let _ = handle.join();
    }
  }

  pub fn size(&self) -> u8 {
    self.size
  }
}

fn main() {
  let mut tasker = Tasker::new(10);
  for i in 0..30 {
    tasker.add(move || {
      println!("Running {}", i);
    });
  }
  println!("tasker get_num_threads:{}", tasker.get_num_threads());
  tasker.execute();
  std::thread::sleep(std::time::Duration::from_millis(1000));
}
