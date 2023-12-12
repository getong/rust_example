use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

const LENGTH: usize = 24;
const BYTES_PER_U32: usize = 4;

fn main() {
  let num: [u32; LENGTH] = [
    1335565270, 4203813549, 2020505583, 2839365494, 2315860270, 442833049, 1854500981, 2254414916,
    4192631541, 2072826612, 1479410393, 718887683, 1421359821, 733943433, 4073545728, 4141847560,
    1761299410, 3068851576, 1582484065, 1882676300, 1565750229, 4185060747, 1883946895, 4146,
  ];
  println!("original_num: {:?}", num);

  let mut bytes = [0u8; LENGTH * BYTES_PER_U32];
  {
    let mut bytes = &mut bytes[..];
    for &n in &num {
      bytes.write_u32::<BigEndian>(n).unwrap();
    }
  }

  let mut num = [0u32; LENGTH];
  {
    let mut bytes = &bytes[..];
    for n in &mut num {
      *n = bytes.read_u32::<BigEndian>().unwrap();
    }
  }

  println!("recovered_num: {:?}", num);
}
