use bytes::BytesMut;
use prost::Message;

mod mypackage {
  include!("mypackage.rs");
}

fn main() {
  // Create a new MyMessage instance
  let mut message = mypackage::MyMessage::default();
  message.content = "Received your message!".to_string();

  // Encode the message into a BytesMut buffer
  let mut buffer = BytesMut::new();
  message.encode_length_delimited(&mut buffer).unwrap();

  // Print the encoded bytes
  println!("buffer: {:?}", buffer);

  let size = message.encoded_len();
  println!("size:{:?}", size);

  println!(
    " encode_length_delimited_to_vec:{:?}",
    message.encode_length_delimited_to_vec()
  );

  println!(" encode_to_vec:{:?}", message.encode_to_vec());
  let mut buf = vec![];
  _ = message.encode(&mut buf);
  println!("buf:{:?}", buf);
  assert_eq!(buf, message.encode_to_vec());

  println!(
    "length_delimiter_len:{:?}",
    prost::length_delimiter_len(10240) as usize
  );

  let mut buffer = BytesMut::new();
  _ = prost::encode_length_delimiter(prost::length_delimiter_len(10240) as usize, &mut buffer);

  println!("encode_length_delimiter: {:?}", buffer);
  // b"\x18\n\x16Received your message!"
  // size:24
  // encode_length_delimited_to_vec:[24, 10, 22, 82, 101, 99, 101, 105, 118, 101, 100, 32, 121, 111, 117, 114, 32, 109, 101, 115, 115, 97, 103, 101, 33]
  // encode_to_vec:[10, 22, 82, 101, 99, 101, 105, 118, 101, 100, 32, 121, 111, 117, 114, 32, 109, 101, 115, 115, 97, 103, 101, 33]
  // buf:[10, 22, 82, 101, 99, 101, 105, 118, 101, 100, 32, 121, 111, 117, 114, 32, 109, 101, 115, 115, 97, 103, 101, 33]
}
