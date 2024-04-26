struct DropHelper {
  int: i32,
}
impl Drop for DropHelper {
  fn drop(&mut self) {
    println!("Dropping DropHelper with int: {}", self.int);
  }
}

fn main() {
  let mut drop_helper = DropHelper { int: 42 };
  let handler = std::thread::spawn(move || {
    println!("Thread spawned");
    drop_helper.int = 43;
  });
  _ = handler.join();
}

// copy from https://github.com/rust-lang/rust/issues/108808
// unread_partial_move_field
