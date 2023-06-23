use bytes::Bytes;
use prost::Message;

mod mypackage {
    include!("mypackage.rs");
}

fn handle_message(message: mypackage::MyMessage) {
    println!("Received MyMessage: {:?}", message);
    // Add your handling logic for MyMessage here
}

pub fn handle_other_message(message: mypackage::OtherMessage) {
    println!("Received OtherMessage: {:?}", message);
    // Add your handling logic for OtherMessage here
}

fn route_message(data: Bytes) {
    if let Ok(my_message) = mypackage::MyMessage::decode(data.clone()) {
        handle_message(my_message);
    } else if let Ok(other_message) = mypackage::OtherMessage::decode(data) {
        handle_other_message(other_message);
    } else {
        eprintln!("Unknown message type");
    }
    // let message = match prost::Message::decode(data.clone()) {
    //     Ok(message) => message,
    //     Err(err) => {
    //         eprintln!("Error decoding message: {}", err);
    //         return;
    //     }
    // };

    // match message {
    //     mypackage::MyMessage { .. } => {
    //         let my_message = match mypackage::MyMessage::decode(data) {
    //             Ok(my_message) => my_message,
    //             Err(err) => {
    //                 eprintln!("Error decoding MyMessage: {}", err);
    //                 return;
    //             }
    //         };
    //         handle_message(my_message);
    //     },
    //     mypackage::OtherMessage { .. } => {
    //         let other_message = match mypackage::OtherMessage::decode(data) {
    //             Ok(other_message) => other_message,
    //             Err(err) => {
    //                 eprintln!("Error decoding OtherMessage: {}", err);
    //                 return;
    //             }
    //         };
    //         handle_other_message(other_message);
    //     },
    // }
}

fn main() {
    // Simulated received data
    let mut buf: Vec<u8> = Vec::new();
    let message = mypackage::MyMessage {
        content: "hello".to_string(),
    };

    if message.encode(&mut buf).is_ok() {
        let message_bytes = Bytes::from(buf);

        route_message(message_bytes);
    }
}
