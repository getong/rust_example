use std::error::Error;
use std::fmt::{self, Display};
use std::fs::File;
use std::io::Read;

fn read_file() -> Result<String, Box<dyn Error>> {
  let mut file = File::open("example.txt")?;
  let mut contents = String::new();
  file.read_to_string(&mut contents)?;
  Ok(contents)
}

// Custom error type
#[derive(Debug)]
struct MyError {
  message: String,
}

impl Display for MyError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "MyError: {}", self.message)
  }
}

impl Error for MyError {}

fn print_error_type(err: &(dyn Error + 'static)) {
  if let Some(io_err) = err.downcast_ref::<std::io::Error>() {
    eprintln!("I/O error occurred: {}", io_err);
  } else if let Some(parse_err) = err.downcast_ref::<std::num::ParseIntError>() {
    eprintln!("Parse error occurred: {}", parse_err);
  } else {
    eprintln!("Unknown error occurred");
  }
}

fn main() {
  // Create a Box<dyn Error> that holds a MyError
  let boxed_error: Box<dyn Error> = Box::new(MyError {
    message: "Something went wrong!".to_string(),
  });

  // Attempt to downcast the error to a MyError reference
  if let Some(my_error) = boxed_error.downcast_ref::<MyError>() {
    println!("Downcast successful! Error message: {}", my_error);
  } else {
    println!("Downcast failed!");
  }

  match read_file() {
    Ok(contents) => println!("File contents: {}", contents),
    Err(err2) => eprintln!("Error: {}", err2),
  }
  let err: Box<dyn Error> = Box::new(std::io::Error::from(std::io::ErrorKind::NotFound));
  print_error_type(&*err);
}

// copy from https://medium.com/@TechSavvyScribe/rust-box-dyn-error-flexible-error-handling-made-easy-245a8e8d1aea
// with help of chatgpt
