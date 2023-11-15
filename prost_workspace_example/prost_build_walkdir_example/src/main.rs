use prost::Message;
use prost::Name;
use std::any::Any;
use std::error::Error;

mod mypackage {
    include!("protos/mypackage.rs");
}

mod protobuf_message_num;

// nc -l 8080
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let message = mypackage::MyMessage {
        content: "Received your message!".to_string(),
    };

    println!("message full_name: {:?}", mypackage::MyMessage::full_name());
    println!("message name: {:?}", mypackage::MyMessage::NAME);
    println!("message package name: {:?}", mypackage::MyMessage::PACKAGE);
    println!("message type_url: {:?}", mypackage::MyMessage::type_url());

    // Serialize the message and send it over the TCP connection
    let bytes = message.encode_to_vec();
    // stream.write_all(&bytes).await?;

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
