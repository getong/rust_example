use nom::{
  bits::{
    bits,
    complete::{tag, take},
  },
  IResult,
};

pub fn take_2_bits(i: (&[u8], usize)) -> IResult<(&[u8], usize), u8> {
  take(2usize)(i)
}

pub fn check_tag(i: (&[u8], usize)) -> IResult<(&[u8], usize), u8> {
  tag(0x01, 1usize)(i)
}

pub fn do_everything_bits(i: (&[u8], usize)) -> IResult<(&[u8], usize), (u8, u8)> {
  let (i, a) = take_2_bits(i)?;
  let (i, b) = check_tag(i)?;
  Ok((i, (a, b)))
}

pub fn do_everything_bytes(i: &[u8]) -> IResult<&[u8], (u8, u8)> {
  bits(do_everything_bits)(i)
}

fn main() {
  println!("Hello, world!");
}
