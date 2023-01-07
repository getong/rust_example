use calamine::{open_workbook, DataType, Reader, Xlsx};

const PATH: &str = "a.xlsx";
const SHEET_NAME: &str = "client_translate";
const COMMENT_PREFIX: &str = "//";
const DEST_DIR: &str = "/Users/gerald/other_project/frontend/src";
//const SHEET_NAME: &str = "server_translate";
//const COMMENT_PREFIX: &str = "--";
//const DEST_DIR : &str = "/Users/gerald/other_project/server/src";

fn main() {
    let mut excel: Xlsx<_> = open_workbook(PATH).unwrap();
    if let Some(Ok(r)) = excel.worksheet_range(SHEET_NAME) {
        for row in r.rows().rev() {
            // read file every two lines
            let mut odd_num = 0;

            let mut whole_line: String = "".to_string();
            let mut chinese_str: String = "".to_string();
            let mut english_str: String = "".to_string();
            odd_num = (odd_num + 1) % 2;
            match odd_num {
                0 => match row.get(1) {
                    Some(DataType::String(filename_and_line)) => {
                        let mut split = filename_and_line.split(": ");
                        if let Some(match_filename) = split.next() {
                            if let Some(line_num_str) = split.next() {
                                // get the match filename and the line_number
                                match line_num_str.parse::<i32>() {
                                    Ok(line_num) => {
                                        // get the match filename and the line name
                                        change_file_with_translate_words(
                                            match_filename,
                                            line_num,
                                            &whole_line,
                                            &chinese_str,
                                            &english_str,
                                        );
                                        println!(
                                            "filename: {}, line_num:{}",
                                            match_filename, line_num
                                        )
                                    }
                                    _ => println!("not match"),
                                }
                            }
                        }
                    }
                    _ => {
                        // not filename and line_num_str
                        println!("line0, not found");
                    }
                },

                _ => {
                    if let Some(DataType::String(origin_whole_line)) = row.get(1) {
                        if let Some(DataType::String(origin_chinese_str)) = row.get(2) {
                            if let Some(DataType::String(origin_english_str)) = row.get(3) {
                                whole_line = origin_whole_line.to_string();
                                chinese_str = origin_chinese_str.to_string();
                                english_str = origin_english_str.to_string();
                            }
                        }
                    }
                }
            }
        }
    }
}

fn change_file_with_translate_words(
    match_filename: &str,
    line_num: i32,
    whole_line: &str,
    chinese_str: &str,
    english_str: &str,
) {
    println!(
        "match_filename: {:?}, line_num:{:?}
, whole_line: {:?}, chinese_str: {:?},  english_str:{:?}",
        match_filename, line_num, whole_line, chinese_str, english_str
    );
}
