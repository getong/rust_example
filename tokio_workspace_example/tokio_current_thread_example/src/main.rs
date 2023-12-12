fn main() {
  // println!("Hello, world!");
  // 创建单一线程的runtime
  let _rt = tokio::runtime::Builder::new_current_thread()
    .build()
    .unwrap();

  std::thread::sleep(std::time::Duration::from_secs(10));
}
