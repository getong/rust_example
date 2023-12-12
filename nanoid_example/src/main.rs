use nanoid::nanoid;

fn main() {
  // println!("Hello, world!");
  let id1 = nanoid!();
  println!("id1 is {}", id1);

  let id2 = nanoid!(10);
  println!("id2 is {}", id2);

  let alphabet: [char; 16] = [
    '1', '2', '3', '4', '5', '6', '7', '8', '9', '0', 'a', 'b', 'c', 'd', 'e', 'f',
  ];

  let id3 = nanoid!(10, &alphabet);
  println!("id3 is {}", id3);
}
