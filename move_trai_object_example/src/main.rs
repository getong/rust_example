trait Named {
  fn name(self: Box<Self>) -> String;
}

struct Person {
  first_name: String,
  last_name: String,
}

impl Named for Person {
  fn name(self: Box<Self>) -> String {
    format!("{} {}", self.first_name, self.last_name)
  }
}

pub struct Mech<'a> {
  driver: Box<dyn Named + 'a>,
}

impl<'a> Mech<'a> {
  pub fn driver_name(self) -> String {
    self.driver.name()
  }
}

fn main() {}
