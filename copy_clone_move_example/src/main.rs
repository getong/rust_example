fn main() {
  println!("Hello, world!");
  move_example();
  copy_example();
  clone_example();
  struct_copy_example();
  struct_clone_example();
}

fn move_example() {
  let str1 = String::from("Rust");
  let str2 = str1; // 所有权被转移（移动）给了 str2，str1 现在已失效。
  println!("{str2}");
  // println!("{str1}"); // 编译期错误：值在移动后被使⽤
}

fn copy_example() {
  let n1 = 32u32;
  let n2 = n1; // 所有权没有转移（未发⽣移动），n1 的值被逐位复制给了 n2。
  println!("n1: {}, n2: {}", n1, n2); // n1 仍然有效
}

fn clone_example() {
  let str1 = String::from("Rust");
  // 显式调⽤ clone。这是⼀个有成本的操作：
  // 它复制栈上的元数据（指针、⻓度和容量），并且在堆上分配新的内存。
  let str2 = str1.clone();
  // 现在两者都是有效的、独⽴拥有各⾃堆数据的所有者。
  println!("str1: {}, str2: {}", str1, str2);
}

// Copy 要求所有字段都实现了 Copy（这里 f64 和 i32 都满足）。
// Copy 是 Clone 的子 trait，所以派生 Copy 时必须同时派生 Clone。
#[derive(Debug, Copy, Clone)]
struct Point {
  x: f64,
  y: f64,
  z: i32,
}

fn struct_copy_example() {
  let p1 = Point {
    x: 1.0,
    y: 2.0,
    z: 3,
  };
  let p2 = p1; // 因为 Point 实现了 Copy，这里是逐位复制，不是移动。
  let p3 = p1.clone(); // 也可以显式 clone，效果和 Copy 一样（逐位复制）。
  // p1 仍然有效，三者互相独立。
  println!("p1: {:?}, p2: {:?}, p3: {:?}", p1, p2, p3);
}

// 含有 String（堆数据）的 struct 不能派生 Copy，只能派生 Clone。
#[derive(Debug, Clone)]
struct Person {
  name: String,
  age: u32,
}

fn struct_clone_example() {
  let person1 = Person {
    name: String::from("Alice"),
    age: 30,
  };
  // 显式调⽤ clone：会深拷贝 name 字段（在堆上分配新内存），age 字段逐位复制。
  let person2 = person1.clone();
  println!("person1: {:?}, person2: {:?}", person1, person2);

  let person3 = person1; // 没有实现 Copy，赋值是移动，person1 从此失效。
  // println!("{:?}", person1); // 编译期错误：值在移动后被使⽤
  println!("person3: {:?}", person3);
}
