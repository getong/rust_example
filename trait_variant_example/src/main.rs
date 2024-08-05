#[trait_variant::make(IntFactory: Send)]
trait LocalIntFactory {
  async fn make(&self) -> i32;
  fn stream(&self) -> impl Iterator<Item = i32>;
  fn call(&self) -> u32;
}

struct Num {
  num: i32,
}

impl LocalIntFactory for Num {
  async fn make(&self) -> i32 {
    self.num
  }

  fn stream(&self) -> impl Iterator<Item = i32> {
    (0 .. self.num).into_iter()
  }

  fn call(&self) -> u32 {
    self.num as u32
  }
}

#[tokio::main]
async fn main() {
  let a = Num { num: 3 };

  let make_num = a.make().await;
  println!("make_num: {}", make_num);

  for i in a.stream() {
    println!("i is {:?}", i);
  }
}
