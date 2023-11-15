use std::fs::create_dir_all;

use walkdir::WalkDir;

fn main() {
    let protobuf_directory = "messages";
    let output_directory = "src/protos";
    _ = create_dir_all(output_directory);

    let proto_files: Vec<_> = WalkDir::new(protobuf_directory)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry.path().is_file() && entry.path().extension() == Some("proto".as_ref())
        })
        .filter_map(|entry| entry.path().to_str().map(String::from))
        .collect();
    println!("proto_files: {:?}", proto_files);

    prost_build::Config::new()
        .out_dir(output_directory)
        .include_file("mod.rs")
        .enable_type_names()
        .compile_protos(&proto_files, &["."])
        .unwrap();
}
