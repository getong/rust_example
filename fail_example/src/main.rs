use fail::{fail_point, FailScenario};

fn do_fallible_work() {
  fail_point!("read-dir");
  let _dir: Vec<_> = std::fs::read_dir(".").unwrap().collect();
  // ... do some work on the directory ...
}

// FAILPOINTS=read-dir=panic cargo run --features fail/failpoints
fn main() {
  let scenario = FailScenario::setup();
  do_fallible_work();
  scenario.teardown();
  println!("done");
}
