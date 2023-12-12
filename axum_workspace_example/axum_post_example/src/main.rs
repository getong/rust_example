use axum::{extract::Json, routing::post, Router};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct Todo {
  id: u32,
  title: String,
  completed: bool,
}

// curl -X POST -H "Content-Type: application/json" -d '{"id":1, "title":"test todo", "completed":false}' http://127.0.0.1:3000/todos
async fn create_todo_handler(Json(todo): Json<Todo>) -> String {
  println!("Created new todo: {:?}", todo);
  format!("Created new todo: {:?}", todo)
}

#[tokio::main]
async fn main() {
  let router = Router::new().route("/todos", post(create_todo_handler));

  let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

  axum::serve(listener, router).await.unwrap();
}
