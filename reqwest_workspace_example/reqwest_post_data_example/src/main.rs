use serde_json::{json, Value};
use std::error::Error;

const URL1: &str = "http://192.168.80.222:8009/statistic-queries";
const URL2: &str = "http://192.168.80.222:8009/deployment-price-count";
const URL3: &str =
  "http://192.168.80.222:8009/user_aggregation/0xbf3a286a477967ebd850cee2dbdbfa6e535a9e64";
const URL4: &str = "http://192.168.80.222:8009/multi_user_aggregation";

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  let client = reqwest::Client::new();

  let json1 = json!({
  "deployment": ["QmbReTnhCweQHmbXxgffkDqkkedo7ojjsUWTKopP1auuTp"],
  "start_date": "2024-09-01",
  });

  let res = client
    .post(URL1)
    .json(&json1)
    .send()
    .await
    .expect("failed to get response")
    .json::<Value>()
    .await
    .expect("failed to get payload");

  println!("{:#?}", res);

  let json2 = json!({
      "indexer": ["0xbf3a286a477967ebd850cee2dbdbfa6e535a9e64"],
      "start_date": "2024-09-01",
  });

  let res = client
    .post(URL1)
    .json(&json2)
    .send()
    .await
    .expect("failed to get response")
    .text()
    .await
    .expect("failed to get payload");

  println!("{:#?}", res);

  let json3 = json!({
      "indexer": ["0xbf3a286a477967ebd850cee2dbdbfa6e535a9e64"],
      "deployment": ["QmbReTnhCweQHmbXxgffkDqkkedo7ojjsUWTKopP1auuTp"],
      "start_date": "2024-09-01",
  });

  let res = client
    .post(URL1)
    .json(&json3)
    .send()
    .await
    .expect("failed to get response")
    .text()
    .await
    .expect("failed to get payload");

  println!("{:#?}", res);

  let json4 = json!({
      "deployment": ["QmbReTnhCweQHmbXxgffkDqkkedo7ojjsUWTKopP1auuTp"],
      "start_date": "2024-09-01",
  });

  let res = client
    .post(URL2)
    .json(&json4)
    .send()
    .await
    .expect("failed to get response")
    .text()
    .await
    .expect("failed to get payload");

  println!("{:#?}", res);

  let res = client
    .get(URL3)
    .send()
    .await
    .expect("failed to get response")
    .text()
    .await
    .expect("failed to get payload");

  println!("{:#?}", res);

  let json4 = json!({
      "user_list": ["0xbf3a286a477967ebd850cee2dbdbfa6e535a9e64", "0xa499b9c52547db14d13216dcd73da0d4d43ba66e", "0x5e15ce35a3821e15d36988d9e0dd181c7c371a07"],
      "start": "2024-09-01",
      "end": "2024-09-02"
  });

  let res = client
    .post(URL4)
    .json(&json4)
    .send()
    .await
    .expect("failed to get response")
    .json::<Value>()
    .await
    .expect("failed to get payload");

  println!("{:#?}", res);

  let json4 = json!({
      "user_list": ["0xbf3a286a477967ebd850cee2dbdbfa6e535a9e64", "0xa499b9c52547db14d13216dcd73da0d4d43ba66e", "0x5e15ce35a3821e15d36988d9e0dd181c7c371a07"],
      "start": "2024-09-13"
  });

  let res = client
    .post(URL4)
    .json(&json4)
    .send()
    .await
    .expect("failed to get response")
    .json::<Value>()
    .await
    .expect("failed to get payload");

  println!("{:#?}", res);

  Ok(())
}
