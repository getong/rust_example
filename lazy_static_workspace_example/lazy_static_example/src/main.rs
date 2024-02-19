use lazy_static::lazy_static;
use std::collections::HashMap;
use std::error::Error;
use std::sync::Mutex;

lazy_static! {
  static ref FRUIT: Mutex<Vec<String>> = Mutex::new(Vec::new());
  static ref PRIVILEGES: HashMap<&'static str, Vec<&'static str>> = {
    let mut map = HashMap::new();
    map.insert("James", vec!["user", "admin"]);
    map.insert("Jim", vec!["user"]);
    map
  };
}

fn insert(fruit: &str) -> Result<(), Box<dyn Error>> {
  let mut db = FRUIT.lock().map_err(|_| "Failed to acquire MutexGuard")?;
  db.push(fruit.to_string());
  Ok(())
}

fn show_access(name: &str) {
  let access = PRIVILEGES.get(name);
  println!("{}: {:?}", name, access);
}

fn main() -> Result<(), Box<dyn Error>> {
  insert("apple")?;
  insert("orange")?;
  insert("peach")?;
  {
    let db = FRUIT.lock().map_err(|_| "Failed to acquire MutexGuard")?;

    db.iter()
      .enumerate()
      .for_each(|(i, item)| println!("{}: {}", i, item));
  }
  insert("grape")?;

  // PRIVILEGES static variable
  let access = PRIVILEGES.get("James");
  println!("James: {:?}", access);
  show_access("Jim");

  Ok(())
}
