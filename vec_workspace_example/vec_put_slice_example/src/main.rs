use bytes::BufMut;

fn main() {
  // Create a new empty vector
  let mut pk_record_key = Vec::new();

  // Append a string slice as bytes
  pk_record_key.put_slice("/pk/".as_bytes());

  // Simulate a peer ID as bytes (for example purposes)
  let peer_id_bytes = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 0];

  // Append the peer ID bytes slice
  pk_record_key.put_slice(peer_id_bytes.as_slice());

  // Print the resulting vector
  println!("{:?}", pk_record_key);
}
