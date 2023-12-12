use skiplist::SkipList;

fn main() {
  // println!("Hello, world!");

  let mut skiplist = SkipList::new();

  skiplist.insert(0, 0);
  skiplist.insert(5, 1);
  assert_eq!(skiplist.len(), 2);
  assert!(!skiplist.is_empty());
  println!("skiplist:{:?}", skiplist);

  let mut skiplist = SkipList::new();
  skiplist.push_front(1);
  skiplist.push_front(2);
  println!("skiplist:{:?}", skiplist);

  let mut skiplist = SkipList::new();
  skiplist.push_back(1);
  skiplist.push_back(2);

  println!("skiplist:{:?}", skiplist);
}
