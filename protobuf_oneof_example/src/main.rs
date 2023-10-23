include!(concat!(env!("OUT_DIR"), "/protos/mod.rs"));
use oneof_example::UnionMessage;
use protobuf::Message;

fn main() {
    // Create a new UnionMessage instance
    let mut union_message = UnionMessage::new();

    // Set the integer field
    union_message.set_integer_value(42);

    // Get the integer field value
    if union_message.has_integer_value() {
        println!("Integer value: {}", union_message.integer_value());
    }
    println!("union_message: {:?}", union_message);

    // Set the string field
    union_message.set_string_value(String::from("Hello, Protobuf!"));

    // Get the string field value
    if union_message.has_string_value() {
        println!("String value: {}", union_message.string_value());
    }
    println!("union_message: {:?}", union_message);

    // Set the boolean field
    union_message.set_bool_value(true);

    // Get the boolean field value
    if union_message.has_bool_value() {
        println!("Boolean value: {}", union_message.bool_value());
    }
    println!("union_message: {:?}", union_message);

    let mut bytes = union_message.write_to_bytes().unwrap();
    println!("bytes: {:?}", bytes);

    let union_message_back = UnionMessage::parse_from_bytes(&mut bytes).unwrap();
    println!("union_message_back: {:?}", union_message_back);
}
