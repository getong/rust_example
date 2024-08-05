// copy from [Queues, Stacks, Deques data structures coded in rust](https://www.alxolr.com/articles/queues-stacks-deques-data-structures-coded-in-rust)

pub fn add(left: usize, right: usize) -> usize {
  left + right
}

pub struct Queue<T: Clone> {
  head: usize,
  tail: usize,
  data: Vec<T>,
}
impl<T: Clone> Queue<T> {
  pub fn new(size: usize) -> Self {
    Queue {
      head: 0,
      tail: 0,
      data: Vec::with_capacity(size),
    }
  }

  pub fn push(&mut self, element: T) {
    self.push_element(element);

    if self.tail + 1 < self.data.capacity() {
      self.tail += 1;
    } else {
      self.tail = 0;
    }
  }

  pub fn pop(&mut self) -> Option<T> {
    let value = self.data.get(self.head);

    if self.head + 1 < self.data.capacity() {
      self.head += 1;
    } else {
      self.head = 0;
    }

    self.wrap_clone(value)
  }

  fn push_element(&mut self, element: T) {
    if self.is_full() {
      self.data.push(element); // grow the vec by pushing an element
    } else {
      self.data[self.tail] = element;
    }
  }

  fn is_full(&self) -> bool {
    self.tail == self.data.len() && self.tail < self.data.capacity()
  }

  fn wrap_clone(&self, element: Option<&T>) -> Option<T> {
    match element {
      Some(el) => Some(el.clone()),
      None => None,
    }
  }
}

pub struct Stack<T: Clone> {
  tail: usize,
  data: Vec<T>,
}

impl<T: Clone> Stack<T> {
  pub fn new(size: usize) -> Self {
    Stack {
      tail: 0,
      data: Vec::with_capacity(size),
    }
  }

  pub fn push(&mut self, element: T) {
    self.push_element(element);

    if self.tail + 1 < self.data.capacity() {
      self.tail += 1;
    } else {
      self.tail = 0;
    }
  }

  pub fn pop(&mut self) -> Option<T> {
    let prev = match self.tail {
      0 => 0,
      _ => {
        self.tail -= 1;
        self.tail
      }
    };

    match self.data.get(prev as usize) {
      Some(value) => Some(value.clone()),
      None => None,
    }
  }

  fn push_element(&mut self, element: T) {
    if self.is_full() {
      self.data.push(element); // grow the vec by pushing an element
    } else {
      self.data[self.tail] = element;
    }
  }

  fn is_full(&self) -> bool {
    self.tail == self.data.len() && self.tail < self.data.len()
  }
}

trait Deque<T> {
  fn push_front(&mut self, element: T);
  fn pop_front(&mut self) -> Option<T>;

  fn push_back(&mut self, element: T);
  fn pop_back(&mut self) -> Option<T>;
}

use std::collections::VecDeque;

use criterion::{criterion_group, criterion_main, Criterion};

pub fn criterion_benchmark(c: &mut Criterion) {
  let mut group = c.benchmark_group("Queue vs VecDeque");
  let size = 100_000;
  let arr = vec![100; size];

  group.bench_function("Queue", |b| {
    let mut queue = Queue::new(size);
    b.iter(|| {
      arr
        .clone()
        .into_iter()
        .for_each(|element| queue.push(element));

      for _ in 0 .. arr.len() {
        queue.pop();
      }
    })
  });

  group.bench_function("VecDeque", |b| {
    let mut queue = VecDeque::new();

    b.iter(|| {
      arr
        .clone()
        .into_iter()
        .for_each(|element| queue.push_back(element));

      for _ in 0 .. arr.len() {
        queue.pop_front();
      }
    })
  });

  group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn it_works() {
    let result = add(2, 2);
    assert_eq!(result, 4);
  }
}
