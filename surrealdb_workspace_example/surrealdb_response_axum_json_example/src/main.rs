use std::{collections::HashMap, sync::Arc};

use axum::{
  Router,
  extract::{Query, State},
  http::StatusCode,
  response::Json,
  routing::get,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use surrealdb::{
  RecordId, Surreal,
  engine::remote::ws::{Client, Ws},
  opt::auth::Root,
};
use tokio::sync::Mutex;

#[derive(Debug, Serialize)]
struct NewUser<'a> {
  name: &'a str,
  balance: &'a str,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct User {
  pub id: RecordId,
  pub name: String,
  pub balance: String,
  pub address: Option<String>,
}

// Shared database state
type DbState = Arc<Mutex<Surreal<Client>>>;

#[tokio::main]
async fn main() {
  let db: Surreal<Client> = Surreal::init();
  let addr = "127.0.0.1:9000";

  // Connect to SurrealDB
  db.connect::<Ws>(addr)
    .await
    .expect("Failed to connect to SurrealDB");

  // Sign in as root user
  db.signin(Root {
    username: "root",
    password: "root",
  })
  .await
  .expect("Failed to authenticate");

  // Select namespace and database
  db.use_ns("test")
    .use_db("test")
    .await
    .expect("Failed to select DB");

  // Create a test user
  let new_user = NewUser {
    name: "John Doe",
    balance: "1000",
  };

  let _: User = db
    .create("user")
    .content(new_user)
    .await
    .unwrap()
    .expect("SurrealDB not connected");

  let db_state = Arc::new(Mutex::new(db));

  // Set up Axum router
  let app = Router::new()
    .route("/users", get(get_users))
    .with_state(db_state);

  println!("Server running on http://127.0.0.1:3000/users");
  let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
  axum::serve(listener, app).await.unwrap();
}

#[derive(Deserialize)]
pub struct Pagination {
  pub page: Option<usize>,
  pub limit: Option<usize>,
}

// Route to fetch all users with pagination
async fn get_users(
  State(db): State<DbState>,
  Query(params): Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
  let db = db.lock().await;

  let page: usize = params.get("page").and_then(|p| p.parse().ok()).unwrap_or(1);
  let limit: usize = params
    .get("limit")
    .and_then(|l| l.parse().ok())
    .unwrap_or(10);
  let start = (page - 1) * limit;

  let query = format!("SELECT * FROM user LIMIT {} START AT {};", limit, start);

  let mut response = match db.query(&query).await {
    Ok(res) => res,
    Err(e) => {
      eprintln!("Query error: {:?}", e);
      return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }
  };

  let users: Vec<User> = match response.take(0) {
    Ok(users) => users,
    Err(e) => {
      eprintln!("Deserialization error: {:?}", e);
      return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }
  };

  Ok(Json(json!({ "users": users })))
}
