use uuid::{NoContext, Timestamp, Uuid, timestamp::context::ContextV7};

fn main() {
  uuid_v7_example();
  uuid_v7_timestamp();
}

fn uuid_v7_example() {
  let id = Uuid::now_v7();
  println!("id is {}", id);
}

fn uuid_v7_timestamp() {
  let ts = Timestamp::from_unix(NoContext, 1497624119, 1234);

  let uuid = Uuid::new_v7(ts);

  assert!(uuid.hyphenated().to_string().starts_with("015cb15a-86d8-7"));

  let context = ContextV7::new();
  let uuid1 = Uuid::new_v7(Timestamp::from_unix(&context, 1497624119, 1234));
  let uuid2 = Uuid::new_v7(Timestamp::from_unix(&context, 1497624119, 1234));

  assert!(uuid1 < uuid2);
}
