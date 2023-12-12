#[derive(Debug)]
struct Person {
  name: String,
  age: Box<u8>,
}

fn tuple_partially_move_example() {
  let mut x = 123;
  let mut y = 456;
  let mut p = (&mut x, &mut y);
  let mut q = p.1;
  // error here, partially moved
  // let mut z = p;
}

fn main() {
  let person = Person {
    name: String::from("Alice"),
    age: Box::new(20),
  };

  // `name` is moved out of person, but `age` is referenced
  let Person { name, ref age } = person;

  println!("The person's age is {}", age);

  println!("The person's name is {}", name);

  // Error! borrow of partially moved value: `person` partial move occurs
  // println!("The person struct is {:?}", person);

  // `person` cannot be used but `person.age` can be used as it is not moved
  println!("The person's age from person struct is {}", person.age);

  // change the name
  let mut person2 = Person {
    name: String::from("Alice"),
    age: Box::new(20),
  };

  let change_name_ptr = &mut person2.name;
  *change_name_ptr = "Bob".to_string();
  assert_eq!("Bob".to_string(), person2.name);

  tuple_partially_move_example();
}
