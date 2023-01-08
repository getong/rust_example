use calamine::{open_workbook, DataType, Reader, Xlsx};

use std::fs;
use std::path::PathBuf;

use std::{
    fs::File,
    io::{BufRead, BufReader},
};

use walkdir::WalkDir;

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
        let mut odd_num = 0;
        let mut whole_line: String = "".to_string();
        let mut chinese_str: String = "".to_string();
        let mut english_str: String = "".to_string();
        for row in r.rows().rev() {
            // read file every two lines
            //println!("row:{:?}", row);

            odd_num = (odd_num + 1) % 2;
            //println!("line:{}, row:{:?}, odd_num:{:?}", line!(), row, odd_num);
            match odd_num {
                0 => match row.get(1) {
                    Some(DataType::String(filename_and_line)) => {
                        // println!("line:sssssss, {}", line!());
                        let mut split = filename_and_line.split(": ");
                        if let Some(match_filename) = split.next() {
                            if let Some(line_num_str) = split.next() {
                                // get the match filename and the line_number
                                match line_num_str.parse::<i32>() {
                                    Ok(line_num) => {
                                        //println!(
                                        //    "filename: {}, line_num:{}",
                                        // match_filename, line_num
                                        //);
                                        // get the match filename and the line name
                                        change_file_with_translate_words(
                                            match_filename,
                                            line_num,
                                            &whole_line,
                                            &chinese_str,
                                            &english_str,
                                        );
                                    }
                                    other => {
                                        println!("line:sssssss, {} , other:{:?}", line!(), other)
                                    }
                                }
                            } else {
                                println!("not match");
                            }
                        }
                    }
                    other => {
                        // not filename and line_num_str
                        println!("other{:?}, not found", other);
                    }
                },

                _ => {
                    //println!("line:sssssss, {}", line!());
                    //println!(
                    //    "row:{:?}, 1 {:?},2 {:?}, 3 {:?}",
                    //row,
                    // row.get(1),
                    // row.get(2),
                    //row.get(3)
                    //);
                    if let Some(DataType::String(origin_whole_line)) = row.get(1) {
                        if let Some(DataType::String(origin_chinese_str)) = row.get(2) {
                            if let Some(DataType::String(origin_english_str)) = row.get(3) {
                                //println!(
                                //  "origin_whole_line:{:?},
                                //origin_chinese_str:{:?},
                                //origin_english_str:{:?}",
                                //  origin_whole_line, origin_chinese_str, origin_english_str
                                //);
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
    //println!(
    //    "match_filename: {:?}, line_num:{:?}
    //, whole_line: {:?}, chinese_str: {:?},  english_str:{:?}",
    //    match_filename, line_num, whole_line, chinese_str, english_str
    //);

    let mut chinese_str_split = chinese_str.split_whitespace();
    let replace_whole_line: String = match chinese_str_split.clone().count() {
        1 => {
            COMMENT_PREFIX.to_string()
                + whole_line
                + "\n"
                + &str::replace(whole_line, chinese_str, english_str)
        }
        chinese_count => {
            let mut english_str_split = english_str.split_whitespace();
            let mut temp_whole_line = whole_line.to_string();
            if english_str_split.clone().count() == chinese_count {
                while let Some(chinese_word) = chinese_str_split.next() {
                    if let Some(english_word) = english_str_split.next() {
                        temp_whole_line =
                            str::replace(&temp_whole_line, chinese_word, english_word);
                    }
                }
                COMMENT_PREFIX.to_string() + whole_line + "\n" + &temp_whole_line
            } else {
                println!(
                    "not_match_not_replace, match_filename: {}  line_num: {}",
                    match_filename, line_num
                );
                // not found
                whole_line.to_string()
                    + "\n"
                    + COMMENT_PREFIX
                    + "wrong_translate_wrong_translate_   "
                    + english_str
            }
        }
    };
    if replace_whole_line != "".to_string() {
        match find_file(match_filename) {
            None => println!("No rusv file was found."),
            Some(filepath) => {
                // println!("Rusv file was found: {:?}", filepath);
                if let Ok(file) = File::open(filepath.clone()) {
                    read_write_line(filepath, file, line_num, replace_whole_line);
                }
                //                if let Ok(file) = File::open(filepath.clone()) {
                //                    let mut reader = BufReader::new(file);
                //                    let mut buf = Vec::new();
                //                    reader.read_to_end(&mut buf).unwrap();
                //                    if let Some(elem) = buf.get_mut((line_num - 1) as usize) {
                //                        *elem = 42;
                //                    }
                //                    File::create(filepath).unwrap().write_all(&buf).unwrap();
                //                }
            }
        }
    }
}

fn read_write_line(path: PathBuf, file: File, line_num: i32, replace_whole_line: String) {
    let reader = BufReader::new(file);
    //let mut lines_string: Vec<String> = vec![];
    // println!("lines:{:?}", lines);
    //for line in reader.lines() {
    //println!("{}", line.as_ref().unwrap());
    //    lines_string.push(line.unwrap());
    //}

    let mut lines_string: Vec<String> = reader
        .lines()
        .map(|l| l.expect("Could not parse line"))
        .collect();

    if let Some(elem) = lines_string.get_mut((line_num - 1) as usize) {
        *elem = replace_whole_line
    }

    //if let Ok(mut file) = File::create(path) {
    //   for i in &lines_string {
    //    _ = writeln!(file, "{}", i);
    //}
    // }
    fs::write(path, lines_string.join("\n")).expect("");
}

fn find_file(filename: &str) -> Option<PathBuf> {
    for entry in WalkDir::new(DEST_DIR)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_name().to_string_lossy().ends_with(filename) {
            return Some(entry.path().to_path_buf());
        }
    }
    return None;
}
