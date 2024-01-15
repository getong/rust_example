#[derive(Clone, Debug, Eq, PartialEq)]
struct SomeErr;

fn main() {
  let x: Result<Option<i32>, SomeErr> = Ok(Some(5));
  let y: Option<Result<i32, SomeErr>> = Some(Ok(5));
  assert_eq!(x.clone().transpose(), y);

  assert_eq!(x, y.transpose());
}
