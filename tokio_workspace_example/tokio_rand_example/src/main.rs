use rand::{
  distributions::{Distribution, Uniform},
  Rng,
};

async fn random_number() -> u64 {
  let mut rng = rand::thread_rng();
  rng.gen_range(1u64 .. 3u64)
}

async fn uniform_number() -> u64 {
  let between = Uniform::from(10 .. 10000);
  let mut rng = rand::thread_rng();

  between.sample(&mut rng)
}

#[tokio::main]
async fn main() {
  loop {
    println!("random_number() is {}", random_number().await);
    println!("random_number() is {}", uniform_number().await);
  }
}
