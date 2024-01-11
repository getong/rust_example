fn main() {
  let x = || {
    let a = String::from("abc");
    println!("{}", a);
  };
  wrap(x);

  let a = String::from("abc");
  let x = || println!("x {}", a);
  wrap2(x); // pass a closure
  wrap2(y); // pass a function
}

fn wrap(c: fn()) {
  c()
}

fn y() {
  println!("y function");
}

fn wrap2(c: impl Fn()) {
  c()
}

// copy from https://stackoverflow.com/questions/52696907/why-does-passing-a-closure-to-function-which-accepts-a-function-pointer-not-work
