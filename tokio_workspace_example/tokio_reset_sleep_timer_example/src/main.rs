use chrono::Local;
use tokio::{self, runtime::Runtime, time};

#[allow(dead_code)]
fn now() -> String {
  Local::now().format("%F %T").to_string()
}

fn main() {
  let rt = Runtime::new().unwrap();
  rt.block_on(async {
    println!("start: {}", now());
    let slp = time::sleep(time::Duration::from_secs(1));
    let mut slp = std::pin::pin!(slp);

    slp
      .as_mut()
      .reset(time::Instant::now() + time::Duration::from_secs(2));

    slp.await;
    println!("end: {}", now());
  });
}
