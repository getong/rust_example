use nom::{bytes::complete::tag, IResult};

fn parse(input: &str) -> IResult<&str, &str> {
  tag("#")(input)
}

fn main() {
  let (remain, pattern) = parse("#ffffff").unwrap();
  println!("the #fffff remain: {}, pattern: {}", remain, pattern);
}
