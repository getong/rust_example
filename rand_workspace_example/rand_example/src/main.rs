use rand::{prelude::IndexedMutRandom, rng, rngs::ThreadRng,  Rng};

fn generate_float(generator: &mut ThreadRng) -> f64 {
  let placeholder: f64 = generator.random();
  return placeholder * 10.0;
}

fn main() {
  println!("Hello, world!");
  let mut choices = [1, 2, 4, 8, 16, 32];
  let mut rng1 = rng();
  println!("{:?}", choices.choose_mut(&mut rng1));
  println!("{:?}", [0, 1, 2, 3, 4, 5].choose_mut(&mut rng1).unwrap());
  assert_eq!(choices[.. 0].choose_mut(&mut rng1), None);

  for _ in 0 .. 5 {
    let random_u16 = rand::random::<u16>();
    print!("{} ", random_u16);
  }
  println!("");

  let mut rng2 = rng();
  let random_number = generate_float(&mut rng2);
  println!("{}", random_number);
}
