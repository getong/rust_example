use axum::{
  extract::Form,
  http::StatusCode,
  response::Html,
  routing::{get, post},
  Router,
};
use serde::Deserialize;

// Define a struct to receive the form data
#[derive(Deserialize)]
struct MyForm {
  username: String,
  age: u8,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  // Create a router with routes for form and submit
  let app = Router::new()
    .route("/", get(show_form))
    .route("/submit", post(process_form));

  // Bind to an address and serve the app
  let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
  axum::serve(listener, app).await?;
  Ok(())
}

// Display the form page
async fn show_form() -> Html<&'static str> {
  Html(
    r#"
        <!DOCTYPE html>
        <html lang="en">
        <body>
            <h1>Submit Your Info</h1>
            <form action="/submit" method="post">
                <label for="username">Username:</label>
                <input type="text" id="username" name="username"><br>
                <label for="age">Age:</label>
                <input type="number" id="age" name="age"><br>
                <button type="submit">Submit</button>
            </form>
        </body>
        </html>
        "#,
  )
}

// Process form submission
async fn process_form(Form(form): Form<MyForm>) -> (StatusCode, String) {
  (
    StatusCode::OK,
    format!(
      "Received: username = {} and age = {}",
      form.username, form.age
    ),
  )
}
