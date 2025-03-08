use serde::{Deserialize, Serialize};
use surrealdb::{
  RecordId, Surreal,
  engine::remote::ws::{Client, Ws},
  opt::auth::Root,
};

#[derive(Debug, Serialize)]
struct NewUser<'a> {
  name: &'a str,
  balance: &'a str,
}

#[derive(Debug, Deserialize)]
struct User {
  id: RecordId,
  name: String,
  balance: String,
  address: Option<String>, // ✅ Fix: Allow None
}

#[tokio::main]
async fn main() -> surrealdb::Result<()> {
  let db: Surreal<Client> = Surreal::init();
  let addr = "127.0.0.1:9000";

  // Create database connection using WebSocket
  db.connect::<Ws>(addr).await?;
  // Sign in as root user
  db.signin(Root {
    username: "root",
    password: "root",
  })
  .await?;

  // Select a specific namespace / database
  db.use_ns("test").use_db("test").await?;

  // Create a new user
  let new_user = NewUser {
    name: "John Doe",
    balance: "1000",
  };
  let created: User = db
    .create("user")
    .content(new_user)
    .await?
    .expect("SurrealDB not connected");
  dbg!("Created user: {:?}", created);

  // Create another user
  let new_user = NewUser {
    name: "john",
    balance: "1000",
  };
  let created: User = db
    .create("user")
    .content(new_user)
    .await?
    .expect("SurrealDB not connected");
  dbg!("Created user: {:?}", created);

  // Query all users
  let mut response = db.query("SELECT * FROM user").await?;
  let users: Vec<User> = response.take(0)?;
  dbg!("Users: {:?}", users);

  // Query for addresses (now safe)
  let mut response = db.query("SELECT address FROM user").await?;
  let addresses: Vec<Option<String>> = response.take(0).unwrap_or_default();
  dbg!("Addresses: {:?}", addresses);

  // Query all users
  let mut response = db.query("SELECT * FROM user").await?;
  let users: Vec<User> = response.take(0)?;
  dbg!("Users: {:?}", users);

  // Fix: Fetch users by correct name field (not `name.first`)
  let mut response = db
    .query("SELECT * FROM user WHERE name = 'John Doe'")
    .await?;
  let users: Vec<User> = response.take(0)?;
  dbg!("Users named 'John Doe': {:?}", users);

  // Fix: Fetch address correctly, handling `None` values
  let mut response = db.query("SELECT address FROM user").await?;
  let addresses: Vec<Option<String>> = response.take(0).unwrap_or_default();
  dbg!("Addresses: {:?}", addresses);

  // Create a new user record
  let new_user = NewUser {
    name: "John Doe",
    balance: "1000",
  };

  let created: User = db
    .create("user")
    .content(new_user)
    .await?
    .expect("SurrealDB not connected");
  dbg!(created);

  // Create a new user record
  let new_user = NewUser {
    name: "john",
    balance: "1000",
  };

  let created: User = db
    .create("user")
    .content(new_user)
    .await?
    .expect("SurrealDB not connected");
  dbg!(created);

  let mut response = db
    // Get `john`'s details
    .query("SELECT * FROM user")
    // List all users whose first name is John
    .query("SELECT * FROM user WHERE name.first = 'John'")
    // Get John's address
    .query("SELECT address FROM user:john")
    // Get all users' addresses
    .query("SELECT address FROM user")
    .await?;

  // Get the first (and only) user from the first query
  let user: Vec<User> = response.take(0)?;
  dbg!("user: {:?}", user);

  // Get all users from the second query
  let users: Vec<User> = response.take(1)?;
  dbg!("users: {:?}", users);

  // Retrieve John's address without making a special struct for it
  let address: Option<String> = response.take((2, "address"))?;
  dbg!("address: {:?}", address);
  // Get all users' addresses
  // let addresses: Option<String> = response.take((3, "address"))?;
  // dbg!("addresses: {:?}", addresses);
  // You can continue taking more fields on the same response
  // object when extracting individual fields
  let mut response = db.query("SELECT * FROM user").await?;
  dbg!("response: {:?}", &response);
  // Since the query we want to access is at index 0, we can use
  // a shortcut instead of `response.take((0, "field"))`
  let ids: Vec<RecordId> = response.take("id")?;
  dbg!("ids: {:?}", ids);
  let names: Vec<String> = response.take("name")?;
  dbg!("names: {:?}", names);
  let addresses: Vec<String> = response.take("address")?;
  dbg!("addresses = {:?}", addresses);

  Ok(())
}
