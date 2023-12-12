use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

struct Person {
  name: String,
  phone: u64,
}

impl Hash for Person {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.name.hash(state);
    self.phone.hash(state);
  }
}

fn main() {
  let person = Person {
    name: "abc".to_string(),
    phone: 123,
  };

  let mut hasher = DefaultHasher::new();

  person.hash(&mut hasher);

  println!("Hash is {:x}!", hasher.finish());
}
