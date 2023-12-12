/*
The smallest i8 is -128 and the biggest i8 is 127.
The smallest u8 is 0 and the biggest u8 is 255.
The smallest i16 is -32768 and the biggest i16 is 32767.
The smallest u16 is 0 and the biggest u16 is 65535.
The smallest i32 is -2147483648 and the biggest i32 is 2147483647.
The smallest u32 is 0 and the biggest u32 is 4294967295.
The smallest i64 is -9223372036854775808 and the biggest i64 is 9223372036854775807.
The smallest u64 is 0 and the biggest u64 is 18446744073709551615.
The smallest i128 is -170141183460469231731687303715884105728 and the biggest i128 is 170141183460469231731687303715884105727.
The smallest u128 is 0 and the biggest u128 is 340282366920938463463374607431768211455.
*/
fn main() {
  println!(
    "The smallest i8 is {} and the biggest i8 is {}.",
    std::i8::MIN,
    std::i8::MAX
  ); // hint: printing std::i8::MIN means "print MIN inside of the i8 section in the standard library"
  println!(
    "The smallest u8 is {} and the biggest u8 is {}.",
    std::u8::MIN,
    std::u8::MAX
  );
  println!(
    "The smallest i16 is {} and the biggest i16 is {}.",
    std::i16::MIN,
    std::i16::MAX
  );
  println!(
    "The smallest u16 is {} and the biggest u16 is {}.",
    std::u16::MIN,
    std::u16::MAX
  );
  println!(
    "The smallest i32 is {} and the biggest i32 is {}.",
    std::i32::MIN,
    std::i32::MAX
  );
  println!(
    "The smallest u32 is {} and the biggest u32 is {}.",
    std::u32::MIN,
    std::u32::MAX
  );
  println!(
    "The smallest i64 is {} and the biggest i64 is {}.",
    std::i64::MIN,
    std::i64::MAX
  );
  println!(
    "The smallest u64 is {} and the biggest u64 is {}.",
    std::u64::MIN,
    std::u64::MAX
  );
  println!(
    "The smallest i128 is {} and the biggest i128 is {}.",
    std::i128::MIN,
    std::i128::MAX
  );
  println!(
    "The smallest u128 is {} and the biggest u128 is {}.",
    std::u128::MIN,
    std::u128::MAX
  );
}
