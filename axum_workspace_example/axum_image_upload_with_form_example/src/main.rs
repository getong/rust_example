use axum::{
  extract::{DefaultBodyLimit, Multipart},
  response::Html,
  routing::get,
  Router,
};
use std::fs::File;
use std::io::Write;
use tower_http::limit::RequestBodyLimitLayer;

#[tokio::main]
async fn main() {
  // build our application with some routes
  let app = Router::new()
    .route("/", get(show_form).post(accept_form))
    .layer(DefaultBodyLimit::disable())
    .layer(RequestBodyLimitLayer::new(
      250 * 1024 * 1024, /* 250mb */
    ))
    .layer(tower_http::trace::TraceLayer::new_for_http());

  // run it with hyper
  let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
    .await
    .unwrap();
  axum::serve(listener, app).await.unwrap();
}

async fn show_form() -> Html<&'static str> {
  Html(
    r#"
        <!doctype html>
        <html>
            <head></head>
            <body>
                <form action="/" method="post" enctype="multipart/form-data">
                    <label>
                        Upload file:
                        <input type="file" name="file" multiple>
                    </label>

                    <input type="submit" value="Upload files">
                </form>
            </body>
        </html>
        "#,
  )
}

async fn accept_form(mut multipart: Multipart) {
  while let Some(field) = multipart.next_field().await.unwrap() {
    let name = field.name().unwrap().to_string();
    let file_name = field.file_name().unwrap().to_string();
    let content_type = field.content_type().unwrap().to_string();
    let data = field.bytes().await.unwrap();

    println!(
      "Length of `{name}` (`{file_name}`: `{content_type}`) is {} bytes",
      data.len()
    );
    // Write the bytes to a file
    let mut file = File::create(file_name).unwrap();
    file.write_all(&data).unwrap();
  }
}
