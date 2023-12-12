use derive_hello2_example::Hello;

#[derive(Hello)]
enum Pet {
  Cat,
}

fn main() {
  // previous code
  let p = Pet::Cat;
  p.hello_world();
}
