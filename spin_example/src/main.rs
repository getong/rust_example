fn main() {
  let s = std::sync::Arc::new(spin::Mutex::new("hello".to_owned()));
  let rs = std::sync::Arc::new(spin::RwLock::new("hello".to_owned()));
  let sc = s.clone();
  let rsc = rs.clone();

  let hdl = std::thread::spawn(move || {
    // 获取锁
    sc.lock().push_str(" thread ");
    rsc.write().push_str(" thread ");
    // 释放锁
  });

  {
    // 获取锁
    s.lock().push_str(" main ");
    {
      let st = rs.read();
      println!("{}", *st);
    }
    rs.write().push_str(" main ");
    // 释放锁
  }

  hdl.join().unwrap();

  println!("{:?}", s);
  println!("{:?}", rs);
}
