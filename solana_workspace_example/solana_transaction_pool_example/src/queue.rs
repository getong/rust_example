use std::{
  cmp::Reverse,
  sync::{
    atomic::{AtomicU64, AtomicUsize, Ordering},
    Arc,
  },
};

use priority_queue::PriorityQueue;
use tokio::sync::{Mutex, Notify};

#[derive(Debug, Clone, Copy)]
pub enum Priority {
  High,
  Normal,
  Low,
}

impl Priority {
  pub fn parse(value: &str) -> Option<Self> {
    match value.trim().to_lowercase().as_str() {
      "high" | "h" => Some(Self::High),
      "low" | "l" => Some(Self::Low),
      "normal" | "n" | "medium" | "m" => Some(Self::Normal),
      _ => None,
    }
  }

  pub fn label(self) -> &'static str {
    match self {
      Self::High => "high",
      Self::Normal => "normal",
      Self::Low => "low",
    }
  }
}

impl std::fmt::Display for Priority {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.label())
  }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
struct PriorityKey {
  level: u8,
  seq: Reverse<u64>,
}

#[derive(Debug)]
struct QueueItem<T> {
  id: u64,
  task: T,
}

impl<T> PartialEq for QueueItem<T> {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
  }
}

impl<T> Eq for QueueItem<T> {}

impl<T> std::hash::Hash for QueueItem<T> {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.id.hash(state);
  }
}

struct QueueCounters {
  len: AtomicUsize,
  drops: AtomicU64,
}

impl QueueCounters {
  fn new() -> Self {
    Self {
      len: AtomicUsize::new(0),
      drops: AtomicU64::new(0),
    }
  }
}

struct WorkerQueue<T> {
  inner: Mutex<PriorityQueue<QueueItem<T>, PriorityKey>>,
  notify: Arc<Notify>,
  max_len: usize,
  seq: AtomicU64,
  counters: Arc<QueueCounters>,
}

impl<T> WorkerQueue<T> {
  fn new(max_len: usize, counters: Arc<QueueCounters>) -> Self {
    Self {
      inner: Mutex::new(PriorityQueue::new()),
      notify: Arc::new(Notify::new()),
      max_len,
      seq: AtomicU64::new(0),
      counters,
    }
  }

  async fn try_pop(&self) -> Option<T> {
    let mut guard = self.inner.lock().await;
    let popped = guard.pop();
    drop(guard);

    if let Some((item, _priority)) = popped {
      self.counters.len.fetch_sub(1, Ordering::Relaxed);
      return Some(item.task);
    }

    None
  }

  async fn push(&self, priority: Priority, item: T) -> Result<(), String> {
    let prev = self.counters.len.fetch_add(1, Ordering::Relaxed);
    if prev >= self.max_len {
      self.counters.len.fetch_sub(1, Ordering::Relaxed);
      self.counters.drops.fetch_add(1, Ordering::Relaxed);
      return Err("priority queue is full".to_string());
    }

    let level = match priority {
      Priority::High => 2,
      Priority::Normal => 1,
      Priority::Low => 0,
    };
    let id = self.seq.fetch_add(1, Ordering::Relaxed);
    let key = PriorityKey {
      level,
      seq: Reverse(id),
    };
    let mut guard = self.inner.lock().await;
    guard.push(QueueItem { id, task: item }, key);
    drop(guard);
    self.notify.notify_one();
    Ok(())
  }
}

pub struct ShardedQueue<T> {
  queues: Vec<Arc<WorkerQueue<T>>>,
  next_idx: AtomicUsize,
  steal_seq: AtomicUsize,
  notify: Arc<Notify>,
}

impl<T> ShardedQueue<T>
where
  T: Send,
{
  pub fn new(worker_count: usize, max_len: usize) -> Self {
    let worker_count = worker_count.max(1);
    let counters = Arc::new(QueueCounters::new());
    let queues = (0 .. worker_count)
      .map(|_| Arc::new(WorkerQueue::new(max_len, Arc::clone(&counters))))
      .collect();
    Self {
      queues,
      next_idx: AtomicUsize::new(0),
      steal_seq: AtomicUsize::new(0),
      notify: Arc::new(Notify::new()),
    }
  }

  pub fn local_notify(&self, worker_id: usize) -> Arc<Notify> {
    self.queues[worker_id].notify.clone()
  }

  pub fn global_notify(&self) -> Arc<Notify> {
    self.notify.clone()
  }

  pub async fn push(&self, priority: Priority, item: T) -> Result<(), String> {
    let idx = self.next_idx.fetch_add(1, Ordering::Relaxed) % self.queues.len();
    self.queues[idx].push(priority, item).await?;
    self.notify.notify_one();
    Ok(())
  }

  pub async fn try_pop(&self, worker_id: usize) -> Option<T> {
    self.queues[worker_id].try_pop().await
  }

  pub async fn steal(&self, worker_id: usize) -> Option<T> {
    let total = self.queues.len();
    if total <= 1 {
      return None;
    }
    let start = self.steal_seq.fetch_add(1, Ordering::Relaxed) % total;
    for offset in 0 .. total {
      let idx = (start + offset) % total;
      if idx == worker_id {
        continue;
      }
      if let Some(task) = self.queues[idx].try_pop().await {
        return Some(task);
      }
    }
    None
  }
}
