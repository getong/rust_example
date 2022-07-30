use std::fs;
use std::io::Read;

use encoding_rs::UTF_16LE;
use encoding_rs_io::DecodeReaderBytesBuilder;

fn main() {
    let file = fs::File::open("./cfg/settings.json").expect("Unable to open file");

    let mut transcoded = DecodeReaderBytesBuilder::new()
        .encoding(Some(UTF_16LE))
        .build(file);

    let mut file_contents = String::new();
    transcoded.read_to_string(&mut file_contents).unwrap();

    println!("result: {}", file_contents);
}
