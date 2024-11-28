fn match_move_option() {
  let opt = Some("hello".to_string());
  match opt {
    Some(x) => println!("Some: x={}", x), // 模式匹配，所有权move
    None => println!("None"),
  }

  // 这里不能再打印
  // println!("{:?}", opt); // 所有权已move
}

fn match_borrow_option_method1() {
  let opt = Some("hello".to_string());

  match &opt {
    Some(x) => println!("{}", x), // 对 &opt 进行模式匹配，此时的 x 是 &String 类型
    None => println!("None"),
  }
  println!("{:?}", opt); // 输出 Some("hello")
}

fn match_borrow_option_method2() {
  let opt = Some("hello".to_string());

  match opt {
    Some(ref x) => println!("{}", x),
    None => println!("None"),
  }
  println!("{:?}", opt);
}

fn match_borrow_option_method3() {
  let opt: &Option<String> = &Some("hello".to_string());

  match opt {
    Some(x) => println!("{}", x),
    None => println!("None"),
  }
  println!("{:?}", opt);
}

// pub const fn as_ref(&self) -> Option<&T> {
//    // 将 opt 的引用&opt 作为参数
//    match *self {
//        // 对 opt 进行模式匹配
//        Some(ref x) => Some(x), // 通过 ref 获取 x 引用，再封装成 Option 返回
//        None => None,
//    }
//}
// convert &Option<String> to be &String
fn option_unwrap(opt: &Option<String>) -> &String {
  match opt {
    Some(x) => x,
    None => panic!("called `Option::unwrap()` on a `None` value"),
  }
}

// convert &mut Option<String> to be &mut String
fn mut_option_unwrap(opt: &mut Option<String>) -> &mut String {
  match opt {
    Some(x) => x,
    None => panic!("called `Option::unwrap()` on a `None` value"),
  }
}

fn main() {
  // println!("Hello, world!");
  match_move_option();
  match_borrow_option_method1();
  match_borrow_option_method2();
  match_borrow_option_method3();

  let opt = Some("hello".to_string());

  let s = option_unwrap(&opt);
  println!("{:?}", s); // hello
  println!("{:?}", opt); // Some("hello")

  let mut opt2 = Some("hello".to_string());

  let s: &mut String = mut_option_unwrap(&mut opt2);
  *s = "world".to_string();
  println!("{:?}", s); // world
  println!("{:?}", opt2); // Some("hello")

  let mut a: String = "abc".to_string();
  let s: &mut String = &mut a;
  *s = "cde".to_string();
  assert_eq!(a, "cde".to_string());
}
