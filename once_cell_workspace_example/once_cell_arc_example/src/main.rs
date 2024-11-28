use std::sync::{Arc, Mutex};

use once_cell::sync::OnceCell;

struct SharedData {
  counter: i32,
}

// cargo run --  --subscriber_port 5000

// 没初始化
static SHARED_DATA: OnceCell<Arc<Mutex<SharedData>>> = OnceCell::new();

use clap::Parser;

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
struct Opt {
  #[arg(long = "subscriber_port", default_value_t = 5000)]
  subscriber_port: i32,
}

fn main() {
  let opt = Opt::parse();
  // 初始化
  SHARED_DATA.get_or_init(|| {
    Arc::new(Mutex::new(SharedData {
      counter: opt.subscriber_port,
    }))
  });

  let data = SHARED_DATA.clone();

  if let Some(data_mutex) = data.get() {
    if let Ok(mut data_lock) = data_mutex.lock() {
      data_lock.counter += 1;

      println!("data_lock.counter:{}", data_lock.counter);
    }
  }
}
