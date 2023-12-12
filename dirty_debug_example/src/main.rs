use dirty_debug::ddbg;

fn main() {
  let state = 1;
  ddbg!("debug_log.log", "Control reached here.  State={}", state);
  println!("Hello, world!");
}
