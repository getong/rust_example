use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Transaction {
  pub from: String,
  pub to: String,
  pub amount: u64,
}

#[derive(Debug)]
enum TransactionError {
  LoadError(std::io::Error),
  ParseError(serde_json::Error),
}

impl From<std::io::Error> for TransactionError {
  fn from(err: std::io::Error) -> Self {
    TransactionError::LoadError(err)
  }
}

impl From<serde_json::Error> for TransactionError {
  fn from(err: serde_json::Error) -> Self {
    TransactionError::ParseError(err)
  }
}

fn main() {
  // println!("Hello, world!");
  let file_name = "transactions.json";
  let txs = get_transactions(file_name);

  match txs {
    Ok(txs) => {
      println!("ts: {:?}", txs);
      if let Some(first_element) = txs.get(0) {
        println!("first_element: {:?}", first_element);
        let parsed = serde_json::json!(first_element);
        println!("parsed: {:?}", parsed);
      }
    }

    Err(err) => match err {
      TransactionError::LoadError(err) => {
        println!("LoadError: {}", err);
      }
      TransactionError::ParseError(err) => {
        println!("ParseError: {}", err);
      }
    },
  }
}

fn get_transactions(file_name: &str) -> Result<Vec<Transaction>, TransactionError> {
  Ok(serde_json::from_str(&std::fs::read_to_string(file_name)?)?)
}
