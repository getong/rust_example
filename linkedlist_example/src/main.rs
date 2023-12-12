use std::collections::LinkedList;

fn main() {
  let mut list_collection = LinkedList::new();
  list_collection.push_back("Our first entry");
  list_collection.push_back("Our second entry");
  list_collection.push_back("Our third entry");
  for entry in list_collection {
    println!("{}", entry);
  }
}
