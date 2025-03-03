use rand::Rng;

/// Define an enum with some variants.
#[derive(Debug, Clone, Copy)]
enum MyEnum {
  Variant1,
  Variant2,
  Variant3,
}

/// Implement random selection for `MyEnum`
impl MyEnum {
  fn random<R: Rng>(rng: &mut R) -> Self {
    match rng.random_range(0 .. 3) {
      0 => MyEnum::Variant1,
      1 => MyEnum::Variant2,
      _ => MyEnum::Variant3,
    }
  }
}

fn main() {
  let mut rng = rand::rng();

  // Generate a random value of `MyEnum`
  let random_value = MyEnum::random(&mut rng);

  println!("Randomly selected enum variant: {:?}", random_value);
}
