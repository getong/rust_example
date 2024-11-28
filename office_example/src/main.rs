// copy from https://docs.rs/office/latest/office/
#![feature(let_chains)]

const SHEET_NAME: &str = "client_translate";
const COMMENT_PREFIX: &str = "//";
// const SHEET_NAME: &str = "server_translate";
// const COMMENT_PREFIX: &str = "--";

use office::{DataType, Excel};

fn main() {
  // opens a new workbook
  let path = "a.xlsx";
  let mut workbook = Excel::open(path).unwrap();

  // Read whole worksheet data and provide some statistics
  if let Ok(range) = workbook.worksheet_range(SHEET_NAME) {
    let mut odd_num = 0;
    let mut match_filename: &str = "";
    let mut line_num: i32 = 0;
    for i in range.rows() {
      odd_num = (odd_num + 1) % 2;
      match odd_num {
        0 => {
          match i.get(1) {
            Some(DataType::String(filename_and_line)) => {
              // println!("line 0, filename_and_line:{:?}", filename_and_line);
              let mut split = filename_and_line.split(':');
              if let Some(match_filename2) = split.next() {
                if let Some(line_str) = split.next() {
                  let mut line_str_split = line_str.split_whitespace();
                  if let Some(line_str) = line_str_split.next() {
                    match line_str.parse::<i32>() {
                      Ok(linenum) => {
                        // 找到对应的文件名和对应的行数
                        match_filename = match_filename2;
                        line_num = linenum;
                      }
                      _ => println!("not match"),
                    }
                  }
                }
              }
            }
            _ => println!("line0, not found"),
          }
        }
        _ => {
          // 找到要替换的文字， i[1]是整行，i[2]是被替换的中文汉字，i[3]是替换的
          // 英文翻译
          // println!("line 1 i[2]:{:?}, i[3]:{:?}", i[2], i[3]);
          if let Some(DataType::String(whole_line)) = i.get(1) {
            if let Some(DataType::String(chinese_str)) = i.get(2) {
              if let Some(DataType::String(english_str)) = i.get(3) {
                let mut chinese_str_split = chinese_str.split_whitespace();
                match chinese_str_split.clone().count() {
                  1 => {
                    // 这里只有一个替换的字符
                    let comment_whole_line = COMMENT_PREFIX.to_string() + whole_line;
                    let replace_whole_line: String =
                      str::replace(whole_line, chinese_str, english_str);

                    let format_str = if whole_line.contains("'") {
                      format!(
                        r##"find . -name {} -exec gsed -i -e '{} a\' -e "{}" -e '{} a\' -e "{}" -e "{}d" {{}} \;"##,
                        match_filename,
                        line_num,
                        comment_whole_line,
                        line_num,
                        replace_whole_line,
                        line_num,
                      )
                    } else {
                      format!(
                        r##"find . -name {} -exec gsed -i -e '{} a\' -e '{}'  -e '{} a\' -e '{}' -e '{}d'  {{}} \;"##,
                        match_filename,
                        line_num,
                        comment_whole_line,
                        line_num,
                        replace_whole_line,
                        line_num
                      )
                    };
                    println!("{}", format_str);
                  }
                  chinese_count => {
                    let mut english_str_split = english_str.split_whitespace();
                    if english_str_split.clone().count() == chinese_count {
                      // 这里有多个替换的字符
                      // gsed -i  -e "7a \" -e "abc" -e "7d" filename
                      let mut replace_whole_line = whole_line.to_string();
                      while let Some(chinese_word) = chinese_str_split.next() {
                        if let Some(english_word) = english_str_split.next() {
                          replace_whole_line =
                            str::replace(&replace_whole_line, chinese_word, english_word);
                        }
                      }
                      let comment_whole_line = COMMENT_PREFIX.to_string() + whole_line;
                      let format_str = if whole_line.contains("'") {
                        format!(
                          r##"find . -name {} -exec gsed -i -e '{} a\' -e "{}" -e '{} a\' -e "{}" -e "{}d" {{}} \;"##,
                          match_filename,
                          line_num,
                          comment_whole_line,
                          line_num,
                          replace_whole_line,
                          line_num,
                        )
                      } else {
                        format!(
                          r##"find . -name {} -exec gsed -i -e '{} a\' -e '{}'  -e '{} a\' -e '{}' -e '{}d'  {{}} \;"##,
                          match_filename,
                          line_num,
                          comment_whole_line,
                          line_num,
                          replace_whole_line,
                          line_num
                        )
                      };

                      println!("{}", format_str);
                    } else {
                      // 对不上数量
                      println!(
                        "filename: {} , line,
{} not match, string:{}, {}, {}",
                        match_filename, line_num, whole_line, chinese_str, english_str
                      );
                    }
                  }
                }
              }
            }
          }
        }
      }
    }
  }
}
