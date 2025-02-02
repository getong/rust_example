use std::{any::Any, error::Error};

use prost::{Message, Name};
// use tokio::io::AsyncWriteExt;
// use tokio::net::TcpStream;

mod protobuf_message_num;

mod mypackage {
  include!("mypackage.rs");
}

#[macro_export]
macro_rules! dynamic_decode {
  ($type:ty, $input:expr) => {
    <$type>::decode($input)
  };
}

// nc -l 8080
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  // let message = mypackage::MyMessage {
  //     content: "hello".to_string(),
  // };
  let message = mypackage::MyMessage {
    content: "Received your message!".to_string(),
  };

  // message full_name: "MyMessage.mypackage"
  // message name: "MyMessage"
  // message package name: "mypackage"
  // message type_url: "/MyMessage.mypackage"
  println!("message full_name: {:?}", mypackage::MyMessage::full_name());
  println!("message name: {:?}", mypackage::MyMessage::NAME);
  println!("message package name: {:?}", mypackage::MyMessage::PACKAGE);
  println!("message type_url: {:?}", mypackage::MyMessage::type_url());

  // let address = "localhost:8080"; // Replace with the server's address
  // let mut stream = TcpStream::connect(address).await?;

  // Serialize the message and send it over the TCP connection
  let bytes = message.encode_to_vec();
  // stream.write_all(&bytes).await?;

  let decode_msg = dynamic_decode!(mypackage::MyMessage, &*bytes).unwrap();
  println!("decode_msg: {:?}", decode_msg);

  let a = protobuf_message_num::decode_by_num(
    *protobuf_message_num::MESSAGE_TO_NUM_LIST
      .get(&mypackage::MyMessage::full_name())
      .unwrap(),
    &bytes,
  );
  println!("a: {:?}", a);

  tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
  println!(
    "num: {:?}",
    protobuf_message_num::MESSAGE_TO_NUM_LIST.get(&mypackage::MyMessage::full_name())
  );

  let get_back_message = protobuf_message_num::decode_by_num(
    *protobuf_message_num::MESSAGE_TO_NUM_LIST
      .get(&mypackage::MyMessage::full_name())
      .unwrap(),
    &bytes,
  )
  .unwrap();
  println!("get_back_message: {:?}", get_back_message);
  let any: Box<dyn Any> = Box::new(get_back_message);

  match any.downcast::<mypackage::MyMessage>() {
    Ok(concrete_instance) => {
      println!("concrete_instance: {:?}", concrete_instance);
    }
    _ => println!("not match"),
  };

  Ok(())
}
