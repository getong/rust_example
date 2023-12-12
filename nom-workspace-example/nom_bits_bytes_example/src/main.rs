use nom::bits::{bits, bytes, streaming::take};
use nom::combinator::rest;
use nom::error::Error;
use nom::sequence::tuple;
use nom::IResult;

fn parse(input: &[u8]) -> IResult<&[u8], (u8, u8, &[u8])> {
  bits::<_, _, Error<(&[u8], usize)>, _, _>(tuple((
    take(4usize),
    take(8usize),
    bytes::<_, _, Error<&[u8]>, _, _>(rest),
  )))(input)
}

fn main() {
  println!("Hello, world!");

  let input = &[0x12, 0x34, 0xff, 0xff];
  assert_eq!(parse(input), Ok((&[][..], (0x01, 0x23, &[0xff, 0xff][..]))));
}
