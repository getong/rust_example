fn apply<F: Fn(&str)>(x: &[&str], f: F) {
  for elem in x {
    f(&elem)
  }
}

fn main() {
  let v = vec!["hello", "world"];
  apply(&v, |x| println!("{}", x));
}
