use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

use std::{borrow::BorrowMut, mem::take};

struct QueueNode<T> {
  value: T,
  next: Option<Box<QueueNode<T>>>,
}

impl<T> QueueNode<T> {
  fn new(value: T) -> QueueNode<T> {
    QueueNode { value, next: None }
  }
}

pub struct Queue<T> {
  end: Option<QueueNode<T>>,
}

impl<T> Queue<T> {
  pub fn new() -> Queue<T> {
    Queue { end: None }
  }

  pub fn is_empty(&self) -> bool {
    self.end.is_none()
  }

  pub fn add(&mut self, value: T) {
    let new_node = QueueNode::new(value);
    if let Some(end) = &mut self.end {
      let mut start = end;
      loop {
        if start.next.is_some() {
          start = (start.next.as_mut().unwrap()).borrow_mut();
        } else {
          break;
        }
      }
      start.next = Some(Box::new(new_node));
    } else {
      self.end = Some(new_node);
    }
  }

  pub fn remove(&mut self) -> Option<T> {
    if !self.is_empty() {
      let end = take(&mut self.end).unwrap();
      if let Some(next) = end.next {
        self.end = Some(*next);
      }
      Some(end.value)
    } else {
      None
    }
  }
}

type Runnable<T> = Box<dyn Fn() -> T + Send>;

pub struct Task<T: Send> {
  runnable: Runnable<T>,
  sender_res: Arc<Mutex<Sender<T>>>,
}

impl<T: Send> Task<T> {
  pub fn new(runnable: Runnable<T>, sender_res: Arc<Mutex<Sender<T>>>) -> Task<T> {
    Task {
      runnable,
      sender_res,
    }
  }
}

pub struct Worker<T: Send> {
  sender_in: Mutex<Sender<Task<T>>>,
  is_running: Arc<AtomicBool>,
}

impl<T: Send + 'static> Worker<T> {
  fn new(sender_out: Sender<()>) -> Worker<T> {
    let (sender_in, receiver) = channel::<Task<T>>();
    let is_running = Arc::new(AtomicBool::new(false));
    let is_running_clone = Arc::clone(&is_running);
    thread::spawn(move || {
      for task in receiver {
        task
          .sender_res
          .lock()
          .unwrap()
          .send((task.runnable)())
          .unwrap();
        is_running_clone.store(false, Ordering::Relaxed);
        sender_out.send(()).unwrap();
      }
    });
    Worker {
      sender_in: Mutex::new(sender_in),
      is_running,
    }
  }

  fn send(&self, task: Task<T>) {
    if let Err(e) = self.sender_in.lock().unwrap().send(task) {
      println!("{}", e);
    }
  }
}

pub struct ThreadPool<T: Send> {
  workers: Arc<Vec<Worker<T>>>,
  tasks: Arc<Mutex<Queue<Task<T>>>>,
}

impl<T: Send + 'static> ThreadPool<T> {
  pub fn new(n_workers: u8) -> ThreadPool<T> {
    let mut _workers = vec![];
    let (sender, receiver) = channel();
    let tasks = Arc::new(Mutex::new(Queue::<Task<T>>::new()));
    for _ in 0 .. n_workers {
      let sender_clone = sender.clone();
      _workers.push(Worker::new(sender_clone));
    }
    let workers = Arc::new(_workers);
    let tasks_copy = Arc::clone(&tasks);
    let workers_copy = Arc::clone(&workers);
    thread::spawn(move || {
      for _ in receiver {
        if let Some(task) = tasks_copy.lock().unwrap().remove() {
          if let Some(worker) = find_free_worker(&workers_copy) {
            worker.send(task);
          }
        }
      }
    });
    ThreadPool { workers, tasks }
  }

  pub fn find_free_worker(&self) -> Option<&Worker<T>> {
    find_free_worker(&self.workers)
  }

  pub fn add(&mut self, task: Task<T>) {
    if let Some(worker) = self.find_free_worker() {
      worker.send(task);
    } else {
      self.tasks.lock().unwrap().add(task);
    }
  }
}

pub fn find_free_worker<T: Send>(workers: &Vec<Worker<T>>) -> Option<&Worker<T>> {
  workers.iter().find(|w| {
    w.is_running
      .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
      .is_ok()
  })
}

fn main() {
  let (sender, receiver) = channel();
  let sender_shared = Arc::new(Mutex::new(sender));
  let mut pool = ThreadPool::new(8);
  for i in 0 .. 1000 {
    pool.add(Task::new(
      Box::new(move || 3 + i),
      Arc::clone(&sender_shared),
    ));
  }
  drop(sender_shared);
  for r in receiver {
    println!("{}", r);
  }
}
