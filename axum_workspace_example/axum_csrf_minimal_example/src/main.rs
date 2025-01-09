use axum::{Form, Router, body::Body, response::IntoResponse, routing::get};
use axum_csrf::{CsrfConfig, CsrfToken, Key};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;

#[derive(Deserialize, Serialize)]
struct Keys {
  authenticity_token: String,
  // Your attributes...
}

#[tokio::main]
async fn main() {
  // initialize tracing
  tracing_subscriber::fmt::init();
  let cookie_key = Key::generate();
  let config = CsrfConfig::default().with_key(Some(cookie_key));

  // build our application with a route
  let app = Router::new()
    // `GET /` goes to `root` and Post Goes to check key
    .route("/", get(root).post(check_key))
    .with_state(config);

  // run our app with hyper
  // `axum::Server` is a re-export of `hyper::Server`
  let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap();
  axum::serve(listener, app).await.unwrap();
}

// basic handler that responds with a static string
async fn root(token: CsrfToken) -> impl IntoResponse {
  let key = Body::new(
    r#"
    <!DOCTYPE html>
<html>

<head>
	<meta charset="UTF-8" />
	<title>Minimal</title>
</head>

<body>
<form method="post" action="/">
    <input type="hidden" name="authenticity_token" value="{{ authenticity_token }}"/>
    <input id="button" type="submit" value="Submit" tabindex="4" />
</form>
</body>
</html>"#
      .replace(
        "{{ authenticity_token }}",
        &token.authenticity_token().unwrap(),
      ),
  );

  // We must return the token so that into_response will run and add it to our response cookies.
  (token, key)
}

async fn check_key(token: CsrfToken, Form(payload): Form<Keys>) -> &'static str {
  if token.verify(&payload.authenticity_token).is_err() {
    "Token is invalid"
  } else {
    "Token is Valid lets do stuff!"
  }
}
