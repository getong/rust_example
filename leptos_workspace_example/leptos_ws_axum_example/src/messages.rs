use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq, Hash)]
pub struct Message {
  text: String,
}
impl Message {
  pub fn new(mut text: String) -> Self {
    text.truncate(500);
    Self { text }
  }
  pub fn text(&self) -> String {
    self.text.clone()
  }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Messages(VecDeque<Message>);

const MESSAGE_LIMIT: usize = 50;

impl Messages {
  pub fn new() -> Self {
    Self { 0: VecDeque::new() }
  }
  pub fn add_message(&mut self, message: Message) {
    if self.0.len() >= MESSAGE_LIMIT {
      self.0.pop_front();
    }
    self.0.push_back(message);
  }
  pub fn get(&self) -> &VecDeque<Message> {
    &self.0
  }
  pub fn len(&self) -> usize {
    self.0.len()
  }
}
