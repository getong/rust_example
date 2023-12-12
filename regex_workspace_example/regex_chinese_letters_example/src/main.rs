use regex::{Regex, RegexSetBuilder};

fn main() {
  // println!("Hello, world!");
  let re = Regex::new(r"(?x)\p{Han}+").unwrap();
  let mat = re.find("//中文字2333").unwrap();
  println!("mat.start(): {}, mat.end():{}", mat.start(), mat.end());

  let regex_set = RegexSetBuilder::new(&[r"[\p{Han}]+"])
    .case_insensitive(true)
    .build()
    .unwrap();
  let line = "中文字1233abc";
  if regex_set.is_match(line) {
    println!("match chinese letters");
  }

  let regex_set2 = RegexSetBuilder::new(&[r"(?x)[\p{Han}]+"])
    .case_insensitive(true)
    .build()
    .unwrap();
  let line = "// 中文字1233abc";
  if regex_set2.is_match(line) {
    println!("match chinese letters");
  }
}
