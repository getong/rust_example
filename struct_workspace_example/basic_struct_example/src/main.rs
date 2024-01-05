#[derive(Debug, PartialEq)]
pub struct Student {
  name: &'static str,
  score: i32,
  option: Option<String>,
}

impl Student {
  pub fn new(name: &'static str, score: i32, option: Option<String>) -> Self {
    Student {
      name,
      score,
      option,
    }
  }

  pub fn get_name(&self) -> &str {
    self.name
  }

  pub fn set_name(&mut self, name: &'static str) {
    self.name = name;
  }

  pub fn get_score(&self) -> i32 {
    self.score
  }
  pub fn set_score(&mut self, score: i32) {
    self.score = score;
  }

  pub fn get_option(&self) -> Option<String> {
    self.option.as_ref().cloned()
  }
  pub fn setoptione(&mut self, option: Option<String>) {
    self.option = option;
  }
}

fn main() {
  // println!("Hello, world!");
  let mut student: Student = Student::new("zhansan", 59, Some("hello".to_owned()));
  println!(
    "name: {}, score: {}",
    student.get_name(),
    student.get_score()
  );

  println!(
    "name: {}, score: {}",
    student.get_name(),
    student.get_score()
  );

  student.set_score(60);
  println!("{:?}", student);

  student.set_score(70);
  println!("{:?}", student);

  println!("option: {:?}", student.get_option());
  println!("option: {:?}", student.get_option());
}
