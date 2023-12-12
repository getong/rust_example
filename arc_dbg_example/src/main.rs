use std::sync::Arc;

fn main() {
  let data: String = "hello".into();

  let s1 = Arc::new(data);
  let s2 = s1.clone();
  let s3 = s1.clone();

  dbg!(&s1 as *const _);
  dbg!(&s2 as *const _);
  dbg!(&s3 as *const _);

  dbg!(s1.as_bytes() as *const _);
  dbg!(s2.as_bytes() as *const _);
  dbg!(s3.as_bytes() as *const _);
}
