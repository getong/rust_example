use nom::bits::{bits, streaming::take};
use nom::error::Error;
use nom::sequence::tuple;
use nom::IResult;

fn parse(input: &[u8]) -> IResult<&[u8], (u8, u8)> {
    bits::<_, _, Error<(&[u8], usize)>, _, _>(tuple((take(4usize), take(8usize))))(input)
}

fn main() {
    // println!("Hello, world!");
    let input = &[0x12, 0x34, 0xff, 0xff];

    let output = parse(input).expect("We take 1.5 bytes and the input is longer than 2 bytes");

    // The first byte is consumed, the second byte is partially consumed and dropped.
    let remaining = output.0;
    assert_eq!(remaining, [0xff, 0xff]);

    let parsed = output.1;
    assert_eq!(parsed.0, 0x01);
    assert_eq!(parsed.1, 0x23);
}
