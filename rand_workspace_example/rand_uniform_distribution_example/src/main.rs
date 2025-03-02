use rand::{
  distr::{Distribution, Uniform},
  rng,
};

fn main() {
  let mut rng = rng();
  let die = Uniform::new_inclusive(1, 6);

  loop {
    let throw = die.expect("not found data").sample(&mut rng);
    println!("Roll the die: {}", throw);
    if throw == 6 {
      break;
    }
  }
}
