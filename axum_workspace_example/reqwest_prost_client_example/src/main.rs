// use reqwest::Client;
use myapp::Todo;
use prost::Message;
use reqwest::blocking::Client;

pub mod myapp {
  include!("myapp.rs");
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
  // Create a `Todo` message
  let todo = Todo {
    id: 42,
    title: "Finish this task".to_string(),
    completed: false,
  };
  // Serialize the message to binary format
  let mut buf: Vec<u8> = Vec::new();
  todo.encode(&mut buf)?;
  println!("buf.into(): {:?}", &buf);

  // Send the message to the server
  let client = Client::new();
  let res = client
    .post("http://127.0.0.1:3000/todos")
    .header("Content-Type", "application/json")
    .body::<Vec<u8>>(buf.into())
    .send()?;

  // Handle the response
  println!("Response status: {:?}", res);
  // let body = res.text()?;
  // println!("Response body: {}", body);

  Ok(())
}
