trait Trait {
  fn abs(self) -> Self;
}

impl Trait for i64 {
  fn abs(self) -> Self {
    2 * self
  }
}

fn main() {
  let x = 42;
  println!("{}", x.abs()); // 84
  println!("{}", x.abs()); // 42
}

// copy from https://rust-lang.zulipchat.com/#narrow/stream/144729-t-types/topic/Method.20resolution.20non-idempotence/near/422734800
