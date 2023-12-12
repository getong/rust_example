fn main() {
  // println!("Hello, world!");
  let mut a = vec![1, 2, 3];
  let mut x = 0;

  std::thread::scope(|s| {
    s.spawn(|| {
      println!("hello from the first scoped thread");
      // We can borrow `a` here.
      // dbg!(&a);
    });
    s.spawn(|| {
      println!("hello from the second scoped thread");
      // We can even mutably borrow `x` here,
      // because no other threads are using it.
      x += a[0] + a[2];
      a[0] = 3;
    });
    println!("hello from the main thread");
  });

  // After the scope, we can modify and access our variables again:
  a.push(4);
  assert_eq!(x, a.len());
  println!("x: {:?}, a:{:?}", x, a);
}
