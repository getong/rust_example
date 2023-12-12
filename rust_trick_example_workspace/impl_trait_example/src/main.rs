// Instead of this

pub fn setup_teardown_generic<A: FnOnce()>(action: A) {
  println!("setting up...");

  action();

  println!("tearing down...")
}

// Use this

fn setup_teardown(action: impl FnOnce()) {
  println!("setting up...");

  action();

  println!("tearing down...")
}

// As a note, this pattern is very useful inside tests
// to create/destroy resources.

fn main() {
  setup_teardown(|| {
    println!("Action!");
  })

  // Output:
  // setting up...
  // Action!
  // tearing down...
}
