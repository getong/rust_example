use merk::*;

fn main() {
  // println!("Hello, world!");
  // load or create a Merk store at the given path
  let mut merk = Merk::open("./merk.db").unwrap();

  // apply some operations
  let batch = [
    (b"key", Op::Put(b"value")),
    (b"key2", Op::Put(b"value2")),
    (b"key3", Op::Put(b"value3")),
    (b"key4", Op::Delete),
  ];
  merk.apply(&batch).unwrap();
}
