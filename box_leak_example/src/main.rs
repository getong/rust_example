#[allow(dead_code)]
#[derive(Debug)]
struct Config {
  a: String,
  b: String,
}
static mut CONFIG: Option<&mut Config> = None;

fn init() -> Option<&'static mut Config> {
  let c = Box::new(Config {
    a: "A".to_string(),
    b: "B".to_string(),
  });

  Some(Box::leak(c))
}

fn main() {
  unsafe {
    CONFIG = init();

    println!("{:?}", CONFIG)
  }
}
