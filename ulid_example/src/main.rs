use ulid::Ulid;

fn main() {
  // Generate a ulid
  let ulid = Ulid::new();

  // Generate a string for a ulid
  let s = ulid.to_string();
  println!("s : {}", s);

  // Create from a String
  let res = Ulid::from_string(&s);
  assert_eq!(ulid, res.unwrap());

  // Or using FromStr
  let res = s.parse();
  assert_eq!(ulid, res.unwrap());
}
