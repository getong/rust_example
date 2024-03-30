use chrono::Local;
use tokio::{runtime::Runtime, time};

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

    //注意调用slp.as_mut().await，而不是slp.await，后者会move消费掉slp
    slp.as_mut().await;
    println!("end 1: {}", now());

    slp
      .as_mut()
      .reset(time::Instant::now() + time::Duration::from_secs(2));

    slp.await;
    println!("end 2: {}", now());
  });
}
