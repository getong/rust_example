use calamine::DataType::Float;
use calamine::DataType::String as OtherString;
use calamine::{open_workbook, DataType, Reader, Xlsx};
use std::io::Write;
use std::string::String;

use std::{
    fs::File,
    io::{BufRead, BufReader},
};

const PATH: &str = "protobuf_list.xlsx";
const SHEET_NAME: &str = "Sheet1";

const RUST_FILE_NAME: &str = "protobuf_message_num.rs";
const DART_FILE_NAME: &str = "protobuf_message_num.dart";

#[tokio::main]
async fn main() {
    let mut list: Vec<Vec<String>> = vec![];
    let mut excel: Xlsx<_> = open_workbook(PATH).unwrap();
    if let Some(Ok(r)) = excel.worksheet_range(SHEET_NAME) {
        for row in r.rows().skip(1) {
            if let Some(Float(number)) = row.get(0) {
                if let Some(OtherString(module_name)) = row.get(1) {
                    if let Some(OtherString(message_name)) = row.get(2) {
                        // println!(
                        //     "number: {number}, module: {module_name}, message_name: {message_name}"
                        // );

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

    // print!("list: {:?}", list);
    if let Ok(mut rust_file) = File::create(RUST_FILE_NAME) {
        if let Ok(mut dart_file) = File::create(DART_FILE_NAME) {
            for i in list.iter() {
                // println!("i: {:?}, {:?}, {:?}", i[0], i[1], i[2]);
                let number = i.get(0).unwrap();
                let package_name = i.get(1).unwrap();
                let message_name = i.get(2).unwrap();
                let mut package_name = package_name.to_owned();
                // println!("i: {:?}, {:?}, {:?}", number, package_name, message_name);
                package_name.push('_');
                package_name.push_str(message_name);
                let new_variable_str = package_name.to_uppercase();
                _ = rust_file
                    .write(format!("const {}: i32 = {};\n", new_variable_str, number).as_bytes());
                _ = dart_file
                    .write(format!("const int {} = {};\n", new_variable_str, number).as_bytes());
            }
        }
    }
}
