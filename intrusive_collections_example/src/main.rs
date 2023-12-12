use intrusive_collections::intrusive_adapter;
use intrusive_collections::{LinkedList, LinkedListLink};
use std::cell::Cell;

// A simple struct containing an instrusive link and a value
struct Test {
  link: LinkedListLink,
  value: Cell<i32>,
}

fn main() {
  // The adapter describes how an object can be inserted into an intrusive
  // collection. This is automatically generated using a macro.
  intrusive_adapter!(TestAdapter = Box<Test>: Test { link: LinkedListLink });

  // Create a list and some objects
  let mut list = LinkedList::new(TestAdapter::new());
  let a = Box::new(Test {
    link: LinkedListLink::new(),
    value: Cell::new(1),
  });
  let b = Box::new(Test {
    link: LinkedListLink::new(),
    value: Cell::new(2),
  });
  let c = Box::new(Test {
    link: LinkedListLink::new(),
    value: Cell::new(3),
  });

  // Insert the objects at the front of the list
  list.push_front(a);
  list.push_front(b);
  list.push_front(c);
  assert_eq!(
    list.iter().map(|x| x.value.get()).collect::<Vec<_>>(),
    [3, 2, 1]
  );

  // At this point, the objects are owned by the list, and we can modify
  // them through the list.
  list.front().get().unwrap().value.set(4);
  assert_eq!(
    list.iter().map(|x| x.value.get()).collect::<Vec<_>>(),
    [4, 2, 1]
  );

  // Removing an object from an instrusive collection gives us back the
  // Box<Test> that we originally inserted into it.
  let a = list.pop_front().unwrap();
  assert_eq!(a.value.get(), 4);
  assert_eq!(
    list.iter().map(|x| x.value.get()).collect::<Vec<_>>(),
    [2, 1]
  );

  // Dropping the collection will automatically free b and c by
  // transforming them back into Box<Test> and dropping them.
  drop(list);
}
