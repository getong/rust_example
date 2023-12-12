fn main() {
  let list1: Vec<u8> = vec![1, 2, 3, 4, 5, 6, 7];

  let list2: Vec<&u8> = list1 // 此处泛型可以理解成自动拷贝
    .iter()
    .filter(|x| x > &&5u8) // 此处的5用了两个'&'
    .collect();

  let list3: Vec<&u8> = list1 // 此处泛型可以理解成自动拷贝
    .iter()
    .filter(|x| **x > 5u8)
    .collect();

  println!(
    "List 1: {:?}\nList 2: {:?} (>5)\nList 3: {:?} (>5)",
    list1, list2, list3
  );
}
