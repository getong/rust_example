use std::{path::PathBuf, time::Duration};

// 注意我们可以用自动导出 Default.
#[derive(Default, Debug)]
struct MyConfiguration {
  // Option defaults to None
  output: Option<PathBuf>,
  // Vecs default to empty vector
  search_path: Vec<PathBuf>,
  // Duration defaults to zero time
  timeout: Duration,
  // bool defaults to false
  check: bool,
}

impl MyConfiguration {
  // add setters here
}

fn main() {
  let default_i8: i8 = Default::default();
  let default_str: String = Default::default();
  let default_bool: bool = Default::default();

  println!("'{}', '{}', '{}'", default_i8, default_str, default_bool);

  // construct a new instance with default values
  let mut conf = MyConfiguration::default();
  // do something with conf here
  conf.check = true;
  println!("conf = {:#?}", conf);

  let conf2: MyConfiguration = Default::default();
  println!("conf2 = {:#?}", conf2);
}
