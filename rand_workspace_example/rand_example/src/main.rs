use rand::{rngs::ThreadRng, seq::SliceRandom, thread_rng, Rng};

fn generate_float(generator: &mut ThreadRng) -> f64 {
  let placeholder: f64 = generator.gen();
  return placeholder * 10.0;
}

fn main() {
  println!("Hello, world!");
  let choices = [1, 2, 4, 8, 16, 32];
  let mut rng = thread_rng();
  println!("{:?}", choices.choose(&mut rng));
  println!("{:?}", [0, 1, 2, 3, 4, 5].choose(&mut rng).unwrap());
  assert_eq!(choices[.. 0].choose(&mut rng), None);

  for _ in 0 .. 5 {
    let random_u16 = rand::random::<u16>();
    print!("{} ", random_u16);
  }
  println!("");

  let mut rng2: ThreadRng = thread_rng();
  let random_number = generate_float(&mut rng2);
  println!("{}", random_number);
}
