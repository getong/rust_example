use std::collections::HashMap;

fn lookup(map: &HashMap<String, i32>, key: &str) -> Result<i32, String> {
  map
    .get(key)
    .copied()
    .ok_or(format!("key {} not found", key))
}

fn lookup_eager(map: &HashMap<i32, i32>, key: i32) -> Result<i32, String> {
  map
    .get(&key)
    .copied()
    .ok_or(format!("key {} not found", key))
}

fn lookup_lazy(map: &HashMap<i32, i32>, key: i32) -> Result<i32, String> {
  map
    .get(&key)
    .copied()
    .ok_or_else(|| format!("key {} not found", key))
}

fn main() -> Result<(), String> {
  // println!("Hello, world!");
  const ERR_DEFAULT: &str = "error message";

  let s = Some("abcde");
  let n: Option<&str> = None;

  let o: Result<&str, &str> = Ok("abcde");
  let e: Result<&str, &str> = Err(ERR_DEFAULT);

  assert_eq!(s.ok_or(ERR_DEFAULT), o); // Some(T) -> Ok(T)
  assert_eq!(n.ok_or(ERR_DEFAULT), e); // None -> Err(default)

  let mut map = HashMap::new();
  map.insert("x".to_string(), 1);
  assert_eq!(lookup(&map, "x"), Ok(1));
  assert_eq!(lookup(&map, "y"), Err("key y not found".to_string()));

  let n = 10000000;
  let mut map = HashMap::new();
  for key in 0..n {
    map.insert(key, key);
  }

  let mut sum = 0;
  for key in 0..n {
    sum += lookup_eager(&map, key)? as i64;
    // sum += lookup_lazy(&map, key)? as i64;
  }

  println!("sum: {}", sum);

  let mut sum = 0;
  for key in 0..n {
    sum += lookup_lazy(&map, key)? as i64;
    // sum += lookup_lazy(&map, key)? as i64;
  }

  println!("sum: {}", sum);
  Ok(())
}
