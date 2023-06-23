fn main() {
    let shirt = prost_build_example::create_large_shirt("White".to_string());
    println!("{:?}", shirt);
    let bytes: Vec<u8> = prost_build_example::serialize_shirt(&shirt);
    println!("{:?}", bytes);
    let shirt = prost_build_example::deserialize_shirt(&bytes);
    println!("{:?}", shirt);
}
