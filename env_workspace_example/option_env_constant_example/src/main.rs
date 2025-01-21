// Example: Using option_env!() to create a constant

// unset MY_ENV_VAR
// cargo run
// set with env
// export MY_ENV_VAR="Hello Rust"; cargo run

const MY_CONSTANT: &str = match option_env!("MY_ENV_VAR") {
  Some(value) => value,    // Use the value from the environment variable if set
  None => "default_value", // Use a default value if the environment variable is not set
};

fn main() {
  println!("MY_CONSTANT: {}", MY_CONSTANT);
}
