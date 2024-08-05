use bitvec::prelude::*;

fn main() {
  // All data-types have macro
  // constructors.
  let _arr = bitarr![u32, Lsb0; 0; 80];
  let _bits = bits![u16, Msb0; 0; 40];

  // Unsigned integers (scalar, array,
  // and slice) can be borrowed.
  let data = 0x2021u16;
  let _bits = data.view_bits::<Msb0>();
  let data = [0xA5u8, 0x3C];
  let bits = data.view_bits::<Lsb0>();

  // Bit-slices can split anywhere.
  let (head, rest) = bits.split_at(4);
  assert_eq!(head, bits[.. 4]);
  assert_eq!(rest, bits[4 ..]);

  // And they are writable!
  let mut data = [0u8; 2];
  let bits = data.view_bits_mut::<Lsb0>();
  // l and r each own one byte.
  let (l, r) = bits.split_at_mut(8);

  // but now a, b, c, and d own a nibble!
  let ((a, b), (c, d)) = (l.split_at_mut(4), r.split_at_mut(4));

  // and all four of them are writable.
  a.set(0, true);
  b.set(1, true);
  c.set(2, true);
  d.set(3, true);

  assert!(bits[0]); // a[0]
  assert!(bits[5]); // b[1]
  assert!(bits[10]); // c[2]
  assert!(bits[15]); // d[3]

  // `BitSlice` is accessed by reference,
  // which means it respects NLL styles.
  assert_eq!(data, [0x21u8, 0x84]);

  // Furthermore, bit-slices can store
  // ordinary integers:
  let eight = [0u8, 4, 8, 12, 16, 20, 24, 28];
  //           a    b  c  d   e   f   g   h
  let mut five = [0u8; 5];
  for (slot, byte) in five
    .view_bits_mut::<Msb0>()
    .chunks_mut(5)
    .zip(eight.iter().copied())
  {
    slot.store_be(byte);
    assert_eq!(slot.load_be::<u8>(), byte);
  }

  assert_eq!(
    five,
    [
      0b0000_0001,
      //  aaaaa bbb
      0b0001_0000,
      //  bb ccccc d
      0b1100_1000,
      //  dddd eeee
      0b0101_0011,
      //  e fffff gg
      0b000_11100,
      //  ggg hhhhh
    ]
  );
}
