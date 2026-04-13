use std::{env, fs::File, io::BufReader};

use mongodb::{
  bson::doc,
  error::Result,
  sync::{Client, Collection},
};
use serde::{Deserialize, Serialize};
use serde_json;

const DEFAULT_MONGODB_URIS: [&str; 2] = [
  "mongodb://mongoadmin:secret@localhost:27017",
  "mongodb://mongoadmin:secret@localhost:27010",
];

#[derive(Serialize, Deserialize)]
struct Person {
  name: String,
  age: i32,
  occupation: String,
  location: String,
  phone: String,
}

fn connect_client() -> Result<Client> {
  if let Ok(uri) = env::var("MONGODB_URI") {
    return connect_client_at(&uri);
  }

  let mut last_error = None;

  for uri in DEFAULT_MONGODB_URIS {
    match connect_client_at(uri) {
      Ok(client) => return Ok(client),
      Err(error) => last_error = Some(error),
    }
  }

  Err(last_error.expect("at least one MongoDB URI candidate should be tried"))
}

fn connect_client_at(uri: &str) -> Result<Client> {
  let client = Client::with_uri_str(uri)?;
  client
    .database("admin")
    .run_command(doc! { "ping": 1 })
    .run()?;
  Ok(client)
}

fn read_records(filename: &str, collection: &Collection<Person>) {
  let file = File::open(filename).unwrap();
  let buf_reader = BufReader::new(file);
  let deserializer = serde_json::Deserializer::from_reader(buf_reader);
  let iterator = deserializer.into_iter::<serde_json::Value>();
  for item in iterator {
    let p: Person = serde_json::from_str(&item.unwrap().to_string()).unwrap();
    println!("Populating data");

    match db_populate(collection, p) {
      Ok(_o) => (),
      Err(e) => println!("Unable to insert data because of {}", e),
    };
  }
}

fn db_populate(collection: &Collection<Person>, record: Person) -> Result<()> {
  let insert_result = collection.insert_one(record).run()?;
  let data_insert_id = insert_result
    .inserted_id
    .as_object_id()
    .expect("Retrieved _id should have been of type ObjectId");
  println!("Inserted ID is {}", data_insert_id);
  Ok(())
}

fn main() {
  const FILENAME: &str = "people.json";
  let client = match connect_client() {
    Ok(client) => client,
    Err(error) => {
      eprintln!(
        "Unable to connect to MongoDB. Set MONGODB_URI if your container uses a different host or \
         port. {}",
        error
      );
      std::process::exit(1);
    }
  };
  let collection = client
    .database("customer_info")
    .collection::<Person>("people");
  read_records(FILENAME, &collection);
}
