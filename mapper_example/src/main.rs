use mapper::Mapper;

fn map_account_id(account_id: &u16) -> String {
  account_id.to_string()
}

#[derive(Mapper)]
#[to(Person)]
struct User {
  #[to(Person, field=_name)]
  pub name: String,
  #[to(Person, with=map_account_id)]
  pub account_id: u16,
  pub age: u8,
}

#[derive(Debug)]
struct Person {
  pub _name: String,
  pub account_id: String,
  pub age: u8,
}

fn main() {
  // println!("Hello, world!");
  let user = User {
    name: "he".to_owned(),
    account_id: 16,
    age: 10,
  };
  let person: Person = user.to();
  println!("person: {:?}", person);
}
