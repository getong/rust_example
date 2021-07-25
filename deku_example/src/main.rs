use deku::prelude::*;

#[derive(Debug, PartialEq, DekuRead, DekuWrite)]
#[deku(endian = "big")]
struct DekuTest {
    #[deku(bits = "4")]
    field_a: u8,
    #[deku(bits = "4")]
    field_b: u8,
    field_c: u16,
}

fn main() {
    let data: Vec<u8> = vec![0b0110_1001, 0xBE, 0xEF];
    let (_rest, mut val) = DekuTest::from_bytes((data.as_ref(), 0)).unwrap();
    assert_eq!(
        DekuTest {
            field_a: 0b0110,
            field_b: 0b1001,
            field_c: 0xBEEF,
        },
        val
    );

    val.field_c = 0xC0FE;

    let data_out = val.to_bytes().unwrap();
    assert_eq!(vec![0b0110_1001, 0xC0, 0xFE], data_out);
}
