extern crate term;
fn main() {
  println!("Hello, world!");
  let mut t = term::stdout().unwrap();
  t.fg(term::color::WHITE).unwrap();
  write!(t, "hello, ").unwrap();

  t.fg(term::color::BLUE).unwrap();
  writeln!(t, "world!").unwrap();

  t.reset().unwrap();
}
