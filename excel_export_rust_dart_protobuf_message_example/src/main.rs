use calamine::DataType::Float;
use calamine::DataType::String as OtherString;
use calamine::{open_workbook, Reader, Xlsx};
use std::collections::HashSet;
use std::fs::File;
use std::io::Write;
use std::string::String;

const PATH: &str = "protobuf_list.xlsx";
const SHEET_NAME: &str = "Sheet1";

const RUST_FILE_NAME: &str = "protobuf_message_num.rs";
const RUST_FILE_INCLUDE_LIST: &[u8] = br#"use once_cell::sync::Lazy;
use prost::{Message, Name};
use std::collections::HashMap;
use std::error::Error;

"#;

const RUST_MESSAGE_TO_NUM_LIST: &[u8] = br#"
pub static MESSAGE_TO_NUM_LIST: Lazy<HashMap<String, i32>> = Lazy::new(|| {
    let mut map = HashMap::new();
"#;
const RUST_MESSAGE_TO_NUM_LIST_END: &[u8] = br#"    map
});
"#;

const RUST_DECODE_BY_NUM_BEGIN: &[u8] = br#"
pub fn decode_by_num(num: i32, bytes: &[u8]) -> Result<Box<dyn Message>, Box<dyn Error>> {
    match num {
"#;

const RUST_DECODE_BY_NUM_END: &[u8] =
  br#"        _ => Err(Box::new(std::io::Error::from(std::io::ErrorKind::NotFound))),
    }
}
"#;

// const RUST_DECODE_FUNCTION_BEGIN: &[u8] = br#"
// pub fn decode_bytes_to_protobuf_data<O>(number: i32, bytes: &[u8]) -> anyhow::Result<O>
//     where
//        O: Message + Default,
// {
//     let out: O = match number {
// "#;

// const RUST_DECODE_FUNCTION_END: &[u8] = br#"        _ => todo!(),
//     };
//     Ok(out)
// }
// "#;

const DART_FILE_NAME: &str = "protobuf_message_num.dart";

const DART_PROTOBUF_MESSAGE_HEADLING: &[u8] =
  b"\nconst Map<Type, int> PROTOBUF_MESSAGE_TYPES = {\n";

const DART_PROTOBUF_MESSAGE_LIST: &[u8] = b"\n/// Builds a [GeneratedMessage] from bytes.
typedef T MessageBuilder<T extends GeneratedMessage>(List<int> bytes);

/// Used to obtain the matching [MessageBuilder] for each defined message code.
final Map<int, MessageBuilder> DART_PROTOBUF_MESSAGE_LIST = <int, MessageBuilder>{\n";

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
      _ = rust_file.write(RUST_FILE_INCLUDE_LIST);

      _ = dart_file.write(DART_FILE_HEADING);
      let mut rust_message_to_number_list: Vec<String> = vec![];
      let mut rust_module_set: HashSet<String> = HashSet::new();
      let mut rust_const_list: Vec<String> = vec![];
      let mut rust_case_list: Vec<String> = vec![];
      // let mut rust_decode_list: Vec<String> = vec![];

      let mut dart_message_to_number_list: Vec<String> = vec![];
      let mut dart_number_to_message_list: Vec<String> = vec![];

      for i in list.iter() {
        let number = i.get(0).unwrap();
        let package_name = i.get(1).unwrap();
        let message_name = i.get(2).unwrap();

        let new_variable_str = (package_name.to_owned() + "_" + message_name).to_uppercase();

        rust_const_list.push(format!("const {}: i32 = {};\n", new_variable_str, number));
        rust_module_set.insert(package_name.to_owned());
        rust_message_to_number_list.push(format!(
          "    map.insert({}::{}::full_name(), {});\n",
          package_name, message_name, new_variable_str,
        ));
        rust_case_list.push(format!(
          "        {} => Ok(Box::new({}::{}::decode(bytes)?)),\n",
          new_variable_str, package_name, message_name,
        ));
        // rust_decode_list.push(format!("        {} => ({}::{} as I)::decode(bytes)?,\n", new_variable_str, package_name, message_name));

        _ = dart_file.write(format!("const int {} = {};\n", new_variable_str, number).as_bytes());

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

      for i in &rust_module_set {
        _ = rust_file
          .write(("mod ".to_owned() + i + " {\n    include!(\"" + i + ".rs\");\n}\n").as_bytes());
      }
      _ = rust_file.write("\n".as_bytes());

      for i in &rust_const_list {
        _ = rust_file.write(i.as_bytes());
      }

      _ = rust_file.write(RUST_MESSAGE_TO_NUM_LIST);
      for i in rust_message_to_number_list.iter() {
        _ = rust_file.write(i.as_bytes());
      }
      _ = rust_file.write(RUST_MESSAGE_TO_NUM_LIST_END);

      _ = rust_file.write(RUST_DECODE_BY_NUM_BEGIN);
      for i in &rust_case_list {
        _ = rust_file.write(i.as_bytes());
      }
      _ = rust_file.write(RUST_DECODE_BY_NUM_END);

      // _ = rust_file.write(RUST_DECODE_FUNCTION_BEGIN);
      // for i in &rust_decode_list{
      //     _ = rust_file.write(i.as_bytes());
      // }
      // _ = rust_file.write(RUST_DECODE_FUNCTION_END);

      _ = dart_file.write(DART_PROTOBUF_MESSAGE_HEADLING);
      for i in dart_message_to_number_list.iter() {
        _ = dart_file.write(i.as_bytes());
      }
      _ = dart_file.write(b"};\n");

      _ = dart_file.write(DART_PROTOBUF_MESSAGE_LIST);
      for i in dart_number_to_message_list.iter() {
        _ = dart_file.write(i.as_bytes());
      }
      _ = dart_file.write(b"};\n");
    }
  }
}
