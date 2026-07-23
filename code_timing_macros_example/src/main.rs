use std::{thread, time::Duration};

use code_timing_macros::{time_function, time_snippet};

#[time_function]
pub(crate) fn sleeper() {
  thread::sleep(Duration::from_secs_f32(2.0f32));
}

#[time_function]
pub fn meaning_of_life() -> u8 {
  42
}

#[time_function]
fn function_with_args(data: &[u8]) -> usize {
  data.len()
}

#[time_function]
async fn test_async() -> Option<u16> {
  let handle = tokio::spawn(async { 10 });

  let out = handle.await.unwrap();
  Some(out)
}

fn main() {
  println!("=== 1. #[time_function] on sleeper() ===");
  sleeper();

  println!("\n=== 2. #[time_function] on meaning_of_life() ===");
  let result = meaning_of_life();
  assert_eq!(result, 42);
  println!("meaning_of_life() returned {result}");

  println!("\n=== 3. #[time_function] with args ===");
  let contents = std::fs::read(std::env::current_exe().expect("failed to get path to self"))
    .expect("failed to read self");
  let _contents_len = function_with_args(&contents);
  println!("Read {} bytes from self", contents.len());

  println!("\n=== 4. #[time_function] on async fn ===");
  let rt = tokio::runtime::Runtime::new().unwrap();
  let out = rt.block_on(test_async());
  println!("test_async() returned {:?}", out.unwrap());

  println!("\n=== 5. time_snippet! (sync) ===");
  time_snippet!({
    let bytes = std::fs::read(std::env::current_exe().unwrap()).unwrap();
    let mut avg = 0.0f32;
    for b in &bytes {
      avg += *b as f32;
    }
    avg /= bytes.len() as f32;
    println!("Avg: {avg}");
  });

  println!("\n=== 6. time_snippet! (async) ===");
  rt.block_on(async {
    time_snippet!(
      async {
        tokio::time::sleep(Duration::from_millis(100)).await;
      }
      .await
    )
  });

  println!("\n=== 7. time_snippet! returning a value ===");
  let result = time_snippet!(100 * 1000 + 20);
  assert_eq!(result, 100 * 1000 + 20);
  println!("time_snippet! returned {result}");

  println!("\n=== 8. #[time_function] on object methods ===");
  let default_version = SomeObject::default();
  let constructed_version = SomeObject::new();

  assert_ne!(default_version.num, constructed_version.num);
  default_version.semi_private();

  println!("\n=== All demonstrations complete! ===");
}

struct SomeObject {
  num: u16,
}

impl SomeObject {
  #[time_function(SomeObject::new())]
  pub fn new() -> Self {
    SomeObject { num: 22 }
  }

  #[time_function(SomeObject::semi_private())]
  pub(crate) fn semi_private(&self) {
    println!("Semi-private function");
    self.private();
  }

  #[time_function(SomeObject::private())]
  fn private(&self) {
    println!("Private function")
  }
}

impl Default for SomeObject {
  #[time_function(SomeObject::default)]
  fn default() -> Self {
    SomeObject { num: 42 }
  }
}
