use std::io;

fn twice(x: &String) -> String {
  format!("{} {}", x, x)
}

fn readin() -> String {
  let mut input = String::new();
  io::stdin().read_line(&mut input).unwrap();
  input.trim().to_string()
}

fn main() {
  let mut x: String = String::from("bogus");
  println!("{}", twice(&x));
  x = readin();
  println!("{}", twice(&x));

  io::copy(&mut io::stdin(), &mut io::stdout()).unwrap();
}
