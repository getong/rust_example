// definition
#[derive(Debug)]
pub struct Queue<T> {
  pub cap: usize,
  pub data: Vec<T>,
}

impl<T> Queue<T> {
  pub fn new(size: usize) -> Self {
    Queue {
      data: Vec::with_capacity(size),
      cap: size,
    }
  }

  pub fn enqueue(&mut self, val: T) -> Result<(), String> {
    if Self::size(&self) == self.cap {
      return Err("No space available".to_string());
    }
    self.data.insert(0, val);
    Ok(())
  }

  pub fn dequeue(&mut self) -> Option<T> {
    if Self::size(&self) > 0 {
      self.data.pop()
    } else {
      None
    }
  }

  pub fn is_empty(&self) -> bool {
    0 == Self::size(&self)
  }

  pub fn size(&self) -> usize {
    self.data.len()
  }
}
