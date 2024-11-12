use axum::{response::Html, routing::get, Router};
use serde::Serialize;

// Define a struct to represent table items
#[derive(Serialize)]
struct Item {
  id: u32,
  name: String,
  value: f64,
}

// Handler function to return the table as an HTML response
async fn get_items_html() -> Html<String> {
  let items = vec![
    Item {
      id: 1,
      name: "Item 1".to_string(),
      value: 10.5,
    },
    Item {
      id: 2,
      name: "Item 2".to_string(),
      value: 20.0,
    },
    Item {
      id: 3,
      name: "Item 3".to_string(),
      value: 30.75,
    },
  ];

  // Generate HTML for the table
  let mut html_table =
    String::from("<table border=\"1\"><tr><th>ID</th><th>Name</th><th>Value</th></tr>");
  for item in items {
    html_table.push_str(&format!(
      "<tr><td>{}</td><td>{}</td><td>{:.2}</td></tr>",
      item.id, item.name, item.value
    ));
  }
  html_table.push_str("</table>");

  Html(html_table)
}

#[tokio::main]
async fn main() {
  // Build the Axum app
  let app = Router::new().route("/items-html", get(get_items_html));

  // Define the address to run the server
  let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

  axum::serve(listener, app).await.unwrap();
}
