pub struct Person {
  name: String,
  pub age: u32,
}

// 这个简单析构函数用于展示析构的触发时机。
impl Drop for Person {
  fn drop(&mut self) {
    println!("Person Drop: {}", self.name);
  }
}

fn main() {
  let _person_a = Person {
    name: String::from("Mr. Hello"),
    age: 23,
  };

  // 子代码块或者函数
  {
    let _person_b = Person {
      name: String::from("Mr. World"),
      age: 24,
    };
    // Person Drop: Mr. World
  }
  // Person Drop: Mr. Hello
}
