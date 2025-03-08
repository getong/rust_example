use serde::{Deserialize, Serialize};
use surrealdb::{
  engine::remote::ws::{Client, Ws},
  opt::auth::Root,
  sql::Thing,
  Surreal,
};

#[derive(Debug, Serialize, Deserialize)]
struct Name<'a> {
  first: &'a str,
  last: &'a str,
}

#[derive(Debug, Serialize, Deserialize)]
struct Person<'a> {
  title: &'a str,
  name: Name<'a>,
  marketing: bool,
}

#[derive(Debug, Serialize)]
struct Responsibility {
  marketing: bool,
}

#[derive(Debug, Deserialize)]
struct Record {
  #[allow(dead_code)]
  id: Thing,
}

#[tokio::main]
async fn main() -> surrealdb::Result<()> {
  let db: Surreal<Client> = Surreal::init();
  let addr = "127.0.0.1:9000";

  // Create database connection using WebSocket (not TCP)
  db.connect::<Ws>(addr).await?;
  // Sign in as root user
  db.signin(Root {
    username: "root",
    password: "root",
  })
  .await?;

  // Select a specific namespace / database
  db.use_ns("test").use_db("test").await?;
  // Create a new person with a random id
  let created: Record = db
    .create("person")
    .content(Person {
      title: "Founder & CEO",
      name: Name {
        first: "Tobie",
        last: "Morgan Hitchcock",
      },
      marketing: true,
    })
    .await?
    .expect("SurrealDB not connected");
  dbg!(created);

  // Update a person record with a specific id
  let updated: Option<Record> = db
    .update(("person", "jaime"))
    .merge(Responsibility { marketing: true })
    .await?;
  dbg!(updated);

  // Select all people records
  let people: Vec<Record> = db.select("person").await?;
  dbg!(people);

  // Perform a custom advanced query
  let groups = db
    .query("SELECT marketing, count() FROM type::table($table) GROUP BY marketing")
    .bind(("table", "person"))
    .await?;
  dbg!(groups);

  Ok(())
}
