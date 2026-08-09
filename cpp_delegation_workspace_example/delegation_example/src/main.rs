// 定义一个接收回调函数的结构体（类似于持有委托的类）
struct Button {
  // 使用 Box<dyn Fn(i32)> 作为动态分发的闭包委托，类似于 C++ 的 std::function<void(int)>
  on_click: Option<Box<dyn Fn(i32)>>,
}

impl Button {
  // 注册委托的方法
  fn set_on_click<F>(&mut self, f: F)
  where
    F: Fn(i32) + 'static,
  {
    self.on_click = Some(Box::new(f));
  }

  // 触发委托
  fn click(&self, count: i32) {
    if let Some(ref callback) = self.on_click {
      callback(count); // 调用委托
    }
  }
}

fn main() {
  let mut btn = Button { on_click: None };

  // 绑定一个闭包作为委托
  btn.set_on_click(|x| {
    println!("按钮被点击了，传入的参数是: {}", x);
  });

  // 触发点击
  btn.click(42);
}
