use std::collections::HashMap;

use yrs::{
  AsyncTransact, Doc, GetString, ReadTxn, Text, Update, types::text::YChange,
  updates::decoder::Decode,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let doc = Doc::new();
  let text = doc.get_or_insert_text("name");

  // every operation in Yrs happens in scope of a transaction
  {
    let mut txn = doc.transact_mut().await;
    text.push(&mut txn, "Hello from yrs!");
    text.format(
      &mut txn,
      11,
      3,
      HashMap::from([("link".into(), "https://github.com/y-crdt/y-crdt".into())]),
    );
  }

  // simulate update with remote peer
  let remote_doc = Doc::new();
  let remote_text = remote_doc.get_or_insert_text("name");

  // in order to exchange data with other documents
  // we first need to create a state vector
  let state_vector = {
    let txn = remote_doc.transact().await;
    txn.state_vector()
  };

  // now compute a differential update based on remote document's
  // state vector
  let bytes = {
    let txn = doc.transact_mut().await;
    txn.encode_diff_v2(&state_vector)
  };

  // both update and state vector are serializable, we can pass them
  // over the wire now apply update to a remote document
  {
    let mut remote_txn = remote_doc.transact_mut().await;
    let update = Update::decode_v2(&bytes)?;
    remote_txn.apply_update(update)?;
  }

  // display raw text (no attributes)
  let remote_txn = remote_doc.transact().await;
  println!("{}", remote_text.get_string(&remote_txn));

  // create sequence of text chunks with optional format attributes
  let diff = remote_text.diff(&remote_txn, YChange::identity);
  for (idx, chunk) in diff.iter().enumerate() {
    println!(
      "chunk {idx}: insert={:?}, attrs={:?}, change={:?}",
      chunk.insert, chunk.attributes, chunk.ychange
    );
  }

  Ok(())
}
