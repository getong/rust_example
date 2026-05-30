use rand::distr::{Distribution, Uniform};
use rand::RngExt;

async fn random_number() -> u64 {
  let mut rng = rand::rng();
  rng.random_range(1u64 .. 3u64)
}

async fn uniform_number() -> u64 {
  let between = Uniform::new(10, 10000).unwrap();
  let mut rng = rand::rng();

  between.sample(&mut rng)
}

#[tokio::main]
async fn main() {
  loop {
    println!("random_number() is {}", random_number().await);
    println!("random_number() is {}", uniform_number().await);
  }
}
