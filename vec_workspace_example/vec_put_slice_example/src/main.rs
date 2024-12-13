fn main() {
  // Create a new vector
  let mut vec = Vec::new();

  // Create a slice to be appended
  let slice = &[1, 2, 3, 4, 5];

  // Append the slice to the vector
  vec.extend_from_slice(slice);

  // Print the vector to see the result
  println!("{:?}", vec);
}
