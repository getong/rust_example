fn box_ref<T>(b: T) -> Box<T> {
  let a = b;
  Box::new(a)
}

#[derive(Copy, Clone, Debug)]
struct Foo;

#[allow(dead_code)]
#[derive(Debug)]
struct Node {
  data: u32,
  next: Option<Box<Node>>,
}

fn main() {
  //println!("Hello, world!");
  let boxed_one: Box<Foo> = Box::new(Foo);
  let unboxed_one: Foo = *boxed_one;
  println!("unboxed_one at {:?} is {:?}", boxed_one, unboxed_one);

  let boxed_two = box_ref(unboxed_one);
  println!("boxed_two at {:p} is {:?} ", boxed_two, *boxed_two);

  let node = Node {
    data: 33,
    next: None,
  };
  println!("node : {:?}", node);
  println!();

  // 将本应存在栈上的地址，存在了堆上
  let mut num = Box::new(1);
  // num_address 指向 box 里面的具体内容（也就是储存在堆上的数值 1）
  let num_address: *mut i32 = &mut *num;
  unsafe { *num_address = 100 }
  // Output: 200
  println!("{}", *num + 100)
}
