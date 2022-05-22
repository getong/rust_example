use nom::bits::complete::take;
use nom::error::Error;
use nom::error::ErrorKind;
use nom::IResult;

// Input is a tuple of (input: I, bit_offset: usize)
fn parser(input: (&[u8], usize), count: usize) -> IResult<(&[u8], usize), u8> {
    take(count)(input)
}

fn main() {
    // Consumes 0 bits, returns 0
    assert_eq!(
        parser(([0b00010010].as_ref(), 0), 0),
        Ok((([0b00010010].as_ref(), 0), 0))
    );

    // Consumes 4 bits, returns their values and increase offset to 4
    assert_eq!(
        parser(([0b00010010].as_ref(), 0), 4),
        Ok((([0b00010010].as_ref(), 4), 0b00000001))
    );

    // Consumes 4 bits, offset is 4, returns their values and increase offset to 0 of next byte
    assert_eq!(
        parser(([0b00010010].as_ref(), 4), 4),
        Ok((([].as_ref(), 0), 0b00000010))
    );

    // Tries to consume 12 bits but only 8 are available
    assert_eq!(
        parser(([0b00010010].as_ref(), 0), 12),
        Err(nom::Err::Error(Error {
            input: ([0b00010010].as_ref(), 0),
            code: ErrorKind::Eof
        }))
    );
}
