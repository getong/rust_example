use byteorder::{BigEndian, ReadBytesExt};
use byteorder::{LittleEndian, WriteBytesExt};
use std::io::Cursor;
// use std::pin::Pin;

fn main() {
    // println!("Hello, world!");
    // one byte for 256
    let mut rdr = Cursor::new(vec![2, 5, 3, 0]);
    // Note that we use type parameters to indicate which kind of byte order
    // we want!
    assert_eq!(517, rdr.read_u16::<BigEndian>().unwrap());
    assert_eq!(&rdr.get_ref()[..], &[2, 5, 3, 0]);
    assert_eq!(768, rdr.read_u16::<BigEndian>().unwrap());
    assert_eq!(&rdr.get_ref()[..], &[2, 5, 3, 0]);

    let mut wtr = vec![];
    wtr.write_u16::<LittleEndian>(517).unwrap();
    // assert_eq!(&Pin::new(&wtr).get_ref()[..], &[5, 2]);
    assert_eq!(&wtr[..], &[5, 2]);
    wtr.write_u16::<LittleEndian>(768).unwrap();
    assert_eq!(wtr, vec![5, 2, 0, 3]);
}
