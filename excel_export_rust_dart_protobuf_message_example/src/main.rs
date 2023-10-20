use calamine::DataType::Float;
use calamine::DataType::String as OtherString;
use calamine::{open_workbook, Reader, Xlsx};
use std::io::Write;
use std::string::String;

use std::fs::File;

const PATH: &str = "protobuf_list.xlsx";
const SHEET_NAME: &str = "Sheet1";

const RUST_FILE_NAME: &str = "protobuf_message_num.rs";
const DART_FILE_NAME: &str = "protobuf_message_num.dart";

const PROTOBUF_MESSAGE_HEADLING: &[u8] = b"\nconst Map<Type, int> PROTOBUF_MESSAGE_TYPES = {\n";

const PROTOBUF_MESSAGE_LIST: &[u8] = b"\n/// Builds a [GeneratedMessage] from bytes.
typedef T MessageBuilder<T extends GeneratedMessage>(List<int> bytes);

/// Used to obtain the matching [MessageBuilder] for each defined message code.
final Map<int, MessageBuilder> PROTOBUF_MESSAGE_LIST = <int, MessageBuilder>{\n";

const DART_FILE_HEADING: &[u8] =b"//use with auto_exporter package\nimport 'export.dart';\nimport 'package:protobuf/protobuf.dart';\n\n";

#[tokio::main]
async fn main() {
    let mut list: Vec<Vec<String>> = vec![];
    let mut excel: Xlsx<_> = open_workbook(PATH).unwrap();
    if let Some(Ok(r)) = excel.worksheet_range(SHEET_NAME) {
        for row in r.rows().skip(1) {
            if let Some(Float(number)) = row.get(0) {
                if let Some(OtherString(module_name)) = row.get(1) {
                    if let Some(OtherString(message_name)) = row.get(2) {
                        let temp_element = vec![
                            number.to_string(),
                            module_name.to_owned(),
                            message_name.to_owned(),
                        ];
                        list.push(temp_element);
                    }
                }
            }
        }
    }

    if let Ok(mut rust_file) = File::create(RUST_FILE_NAME) {
        if let Ok(mut dart_file) = File::create(DART_FILE_NAME) {
            _ = dart_file.write(DART_FILE_HEADING);
            let mut dart_message_to_number_list: Vec<String> = vec![];
            let mut dart_number_to_message_list: Vec<String> = vec![];

            for i in list.iter() {
                let number = i.get(0).unwrap();
                let package_name = i.get(1).unwrap();
                let message_name = i.get(2).unwrap();
                let mut owned_package_name = package_name.to_owned();

                owned_package_name.push('_');
                owned_package_name.push_str(message_name);
                let new_variable_str = owned_package_name.to_uppercase();
                _ = rust_file
                    .write(format!("const {}: i32 = {};\n", new_variable_str, number).as_bytes());
                _ = dart_file
                    .write(format!("const int {} = {};\n", new_variable_str, number).as_bytes());
                // let complete_message_name = package_name.to_owned() + "." + message_name;
                dart_message_to_number_list
                    .push("    ".to_owned() + message_name + ": " + &new_variable_str + ",\n");
                dart_number_to_message_list.push(
                    "    ".to_owned()
                        + &new_variable_str
                        + ": (List<int> bytes) => "
                        + message_name
                        + ".fromBuffer(bytes),\n",
                );
            }

            _ = dart_file.write(PROTOBUF_MESSAGE_HEADLING);
            for i in dart_message_to_number_list.iter() {
                _ = dart_file.write(i.as_bytes());
            }
            _ = dart_file.write(b"};\n");

            _ = dart_file.write(PROTOBUF_MESSAGE_LIST);
            for i in dart_number_to_message_list.iter() {
                _ = dart_file.write(i.as_bytes());
            }
            _ = dart_file.write(b"};\n");
        }
    }
}
