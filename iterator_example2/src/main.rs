struct MyStruct {
    data: Vec<i32>,
}

impl MyStruct {
    fn new(data: Vec<i32>) -> Self {
        MyStruct { data }
    }
}

// Implement the IntoIterator trait for MyStruct
impl<'a> IntoIterator for &'a MyStruct {
    type Item = &'a i32;
    type IntoIter = std::slice::Iter<'a, i32>;

    fn into_iter(self) -> Self::IntoIter {
        self.data.iter()
    }
}

// Implement the IntoIterator trait for &mut MyStruct
impl<'a> IntoIterator for &'a mut MyStruct {
    type Item = &'a mut i32;
    type IntoIter = std::slice::IterMut<'a, i32>;

    fn into_iter(self) -> Self::IntoIter {
        self.data.iter_mut()
    }
}

// Implement the IntoIterator trait for MyStruct
impl IntoIterator for MyStruct {
    type Item = i32;
    type IntoIter = std::vec::IntoIter<i32>;

    fn into_iter(self) -> Self::IntoIter {
        self.data.into_iter()
    }
}

// Define a struct
struct MyStruct2 {
    name: String,
    age: u8,
}

// Implementation of the struct
impl MyStruct2 {
    // Constructor function
    fn new(name: String, age: u8) -> Self {
        MyStruct2 { name, age }
    }
}

// Implementation of IntoIterator trait for MyStruct
impl IntoIterator for MyStruct2 {
    type Item = (String, u8);
    type IntoIter = std::iter::Once<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        std::iter::once((self.name, self.age))
    }
}

fn main() {
    let mut my_struct = MyStruct::new(vec![1, 2, 3, 4, 5]);

    // Using the custom struct in a for loop using an immutable reference
    for item in &my_struct {
        println!("Current item: {}", item);
    }

    // Using the custom struct in a for loop with a mutable reference
    for item in &mut my_struct {
        *item *= 2; // Modify the value of each element
        println!("Current item: {}", item);
    }

    // Using the custom struct in a for loop without referencing it
    for item in my_struct {
        println!("Current item: {}", item);
    }

    // Create an instance of MyStruct
    let my_struct = MyStruct2::new("John".to_string(), 30);

    // Use the for-loop to iterate over the struct
    for (name, age) in my_struct {
        // Print the values in the tuple
        println!("Name: {}", name);
        println!("Age: {}", age);
    }
}
