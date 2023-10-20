// fn main() {
//     tonic_build::compile_protos("message.proto").unwrap();
// }

fn main() {
    let mut config = prost_build::Config::new();
    config
        .out_dir("src")
        .enable_type_names()
        .compile_protos(&["src/message.proto"], &["."])
        .unwrap();
}
