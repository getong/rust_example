use serde::{Deserialize, Deserializer};
use serde_json;

#[derive(Deserialize)]
struct ComputeUnitItem {
  name: String,
  #[serde(deserialize_with = "str_to_u64")]
  value: u64,
}

fn str_to_u64<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
  D: Deserializer<'de>,
{
  let s = String::deserialize(deserializer)?;
  let mut value = s.parse::<u64>().map_err(serde::de::Error::custom)?;
  if value == 0 {
    value = 1; // Change 0 to 1
  }
  Ok(value)
}

fn main() {
  let json_data = r#"
        {
            "name": "example",
            "value": "0"
        }
    "#;
  let item: ComputeUnitItem = serde_json::from_str(json_data).unwrap();
  println!("Name: {}, Value: {}", item.name, item.value);

  let json_data = r#"
        {
            "name": "example",
            "value": "12345"
        }
    "#;
  let item: ComputeUnitItem = serde_json::from_str(json_data).unwrap();
  println!("Name: {}, Value: {}", item.name, item.value);
}
