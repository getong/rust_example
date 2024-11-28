use std::{cell::RefCell, rc::Rc};
// use std::collections::LinkedList;

type SingleLink = Option<Rc<RefCell<Node>>>;

#[derive(Clone)]
pub struct Node {
  value: String,
  next: SingleLink,
}

pub struct TransactionLog {
  head: Option<Rc<RefCell<Node>>>,
  tail: Option<Rc<RefCell<Node>>>,
  pub length: u64,
}

impl Node {
  fn new(value: String) -> Rc<RefCell<Node>> {
    Rc::new(RefCell::new(Node { value, next: None }))
  }
}

impl TransactionLog {
  pub fn new_empty() -> TransactionLog {
    TransactionLog {
      head: None,
      tail: None,
      length: 0,
    }
  }

  pub fn append(&mut self, value: String) {
    let new = Node::new(value);
    match self.tail.take() {
      Some(old) => old.borrow_mut().next = Some(new.clone()),
      None => self.head = Some(new.clone()),
    };
    self.length += 1;
    self.tail = Some(new);
  }

  pub fn pop(&mut self) -> Option<String> {
    self.head.take().map(|head| {
      if let Some(next) = head.borrow_mut().next.take() {
        self.head = Some(next);
      } else {
        self.tail.take();
      }
      self.length -= 1;
      Rc::try_unwrap(head)
        .ok()
        .expect("Something is terribly wrong")
        .into_inner()
        .value
    })
  }
}

fn main() {
  println!("Hello, world!");
}
