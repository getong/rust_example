use serde::{Deserialize, Serialize};

// We will serialize and deserialize instances of
// this struct
#[derive(Serialize, Deserialize, Debug)]
struct ServerConfig {
  workers: u64,
  ignore: bool,
  auth_server: Option<String>,
}

fn main() {
  let config = ServerConfig {
    workers: 100,
    ignore: false,
    auth_server: Some("auth.server.io".to_string()),
  };
  {
    println!("To and from YAML");
    let serialized = serde_yaml::to_string(&config).unwrap();
    println!("{}", serialized);
    let deserialized: ServerConfig = serde_yaml::from_str(&serialized).unwrap();
    println!("{:?}", deserialized);
  }

  println!("\n\n");
  {
    println!("To and from JSON");
    let serialized = serde_json::to_string(&config).unwrap();
    println!("{}", serialized);
    let deserialized: ServerConfig = serde_json::from_str(&serialized).unwrap();
    println!("{:?}", deserialized);
  }

  println!("\n\n");
  {
    println!("To and from binary by using serde_json");
    let serialized = serde_json::to_string(&config)
      .unwrap()
      .as_bytes()
      .to_owned();
    println!("{:?}\nlength is {}", serialized, serialized.len());
    let deserialized: ServerConfig =
      serde_json::from_str(&String::from_utf8(serialized).unwrap()).unwrap();
    println!("{:?}", deserialized);
  }

  println!("\n\n");
  {
    println!("To and from binary by using bincode");

    // Serialize the config struct to a binary format
    let serialized = bincode::serialize(&config).unwrap();
    println!(
      "Serialized binary: {:?}\nlength is {}",
      serialized,
      serialized.len()
    );

    // Deserialize the binary data back into a ServerConfig struct
    let deserialized: ServerConfig = bincode::deserialize(&serialized).unwrap();
    println!("Deserialized struct: {:?}", deserialized);
  }
}
