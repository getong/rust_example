use tikv_client::{RawClient, TransactionClient};
use tokio;

#[tokio::main]
async fn main() {
  // Create a new raw client
  let client = RawClient::new(vec!["127.0.0.1:2379"]).await.unwrap();

  // Set a key-value pair
  let key = b"key1".to_vec();
  let value = b"value1".to_vec();
  client.put(key.clone(), value.clone()).await.unwrap();

  // Get the value of the key
  let retrieved_value = client.get(key.clone()).await.unwrap();
  println!("Retrieved value: {:?}", retrieved_value);

  // Delete the key-value pair
  client.delete(key.clone()).await.unwrap();

  // Try to get the value again to confirm deletion
  let retrieved_value = client.get(key).await.unwrap();
  println!("Retrieved value after deletion: {:?}", retrieved_value);

  let txn_client = TransactionClient::new(vec!["127.0.0.1:2379"])
    .await
    .unwrap();
  let mut txn = txn_client.begin_optimistic().await.unwrap();
  txn.put("key".to_owned(), "value".to_owned()).await.unwrap();
  let value = txn.get("key".to_owned()).await.unwrap();
  println!("Retrieved value: {:?}", value);
  txn.commit().await.unwrap();
}
