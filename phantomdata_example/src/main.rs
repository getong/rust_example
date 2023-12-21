use std::marker::PhantomData;

// A generic struct holding a slice of elements of type T
struct SliceContainer<'a, T> {
  elements: &'a [T],
  phantom: PhantomData<&'a T>,
}

// Implementation of SliceContainer
impl<'a, T: std::fmt::Debug> SliceContainer<'a, T> {
  // Constructor for SliceContainer
  fn new(elements: &'a [T]) -> Self {
    SliceContainer {
      elements,
      phantom: PhantomData,
    }
  }

  // A method that prints each element in the slice
  fn print_elements(&self) {
    for element in self.elements {
      println!("{:?}", element);
    }
  }
}

// Example usage
fn main() {
  // Create a SliceContainer with a slice of i32
  let slice_i32 = &[1, 2, 3, 4, 5];
  let container_i32 = SliceContainer::new(slice_i32);
  container_i32.print_elements();

  // Create a SliceContainer with a slice of Strings
  let slice_strings = &["apple", "banana", "cherry"];
  let container_strings = SliceContainer::new(slice_strings);
  container_strings.print_elements();
}
