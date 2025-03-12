use uuid::Uuid;

fn main() {
  uuid_v7_example();
}

fn uuid_v7_example() {
  let id = Uuid::now_v7();
  println!("id is {}", id);
}
