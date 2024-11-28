// fn main() {
//     println!("Hello, world!");
// }
use std::fmt::{Debug, Display};
// use std::iter::Sum;
use std::mem::size_of;
// use std::ops::{Add, Mul};
fn main() {
  // This is how we specify the type generic type explicitly.
  // In this code rust won't be able to infer the type themselves.
  const_generics1::<true, &str>("string");
  const_generics1::<false, i32>(67);

  // In this code rust is able to infer both the type and value.
  const_generics2([1, 2, 3, 4]);
  const_generics2(['a', 'b', 'c']); //[i32;4]
  let _m: [char; 3] = ['a', 'b', 'a']; //[char;3]

  // let unsize1: ToString;
  // let unsize2: Fn(&str) -> bool;

  // Any type that implements Display trait.
  // let sized1: &dyn ToString;
  // sized1 = &45;
  // let sized2: Box<String>;
  // sized2 = Box::new(String::from(
  //     "Bytes are counted for memory-constrained devices",
  // ));
  println!("{:?}", size_of::<&dyn ToString>());
  println!("{:?}", size_of::<Box<dyn Fn(&str) -> bool>>());
  println!("{:?}", size_of::<Box<String>>());
}
fn const_generics1<const A: bool, T: Display>(i: T) {
  if A {
    println!("This is True");
  } else {
    println!("This is false");
  }
  println!("{}", i);
}
fn const_generics2<T, const N: usize>(i: [T; N])
where
  T: Debug,
{
  println!("{:?}", i);
}

// copy from [Generics and Const Generics in Rust](https://sanjuvi.github.io/Blog/posts/Rust%20Generics/)
