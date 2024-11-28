fn main() -> Result<(), Box<dyn std::error::Error>> {
  // println!("Hello, world!");

  let f = std::fs::File::open("something.yaml")?;
  let d: String = serde_yaml::from_reader(f)?;
  println!("Read YAML string: {}", d);
  Ok(())
}
