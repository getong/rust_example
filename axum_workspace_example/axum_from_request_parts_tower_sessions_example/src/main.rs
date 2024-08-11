use axum::{
  async_trait, extract::FromRequestParts, http::request::Parts, response::Html, routing::get,
  Router,
};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use time::Duration;
use tower_sessions::{cookie::Key, Expiry, MemoryStore, Session, SessionManagerLayer};

// # Set session and store cookies in a file
// curl -c /tmp/cookies1.txt localhost:3000/set
// # Access the home page with the stored cookies
// curl -b /tmp/cookies1.txt localhost:3000
// Hello, user_2275!

// # Set session and store cookies in a file
// curl -c /tmp/cookies2.txt localhost:3000/set
// # Access the home page with the stored cookies
// curl -b /tmp/cookies2.txt localhost:3000
// Hello, user_4250!
// tower_sessions is thread safe

// Define a struct for storing session data
#[derive(Serialize, Deserialize, Debug, Clone)]
struct MySessionData {
  username: String,
}

#[tokio::main]
async fn main() {
  // Create a memory store for the sessions
  let key = Key::generate();

  let session_store = MemoryStore::default();
  let session_layer = SessionManagerLayer::new(session_store)
    .with_signed(key)
    .with_secure(false)
    .with_expiry(Expiry::OnInactivity(Duration::seconds(10)));

  // Build the application with routes and the session layer
  let app = Router::new()
    .route("/", get(home_handler))
    .route("/set", get(set_session_handler))
    .layer(session_layer);

  // Start the server
  let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
  println!("Listening on {}", addr);
  let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
  axum::serve(listener, app.into_make_service())
    .await
    .unwrap();
}

async fn set_session_handler(session: Session) -> Html<&'static str> {
  // Generate a random username
  let random_username: String = generate_random_username();

  let data = MySessionData {
    username: random_username,
  };

  match session.insert("user_data", &data).await {
    Ok(_) => {
      println!("Session data set successfully.");
      Html("Session data set. Go to the home page to see it.")
    }
    Err(_) => {
      println!("Failed to set session data.");
      Html("Failed to set session data.")
    }
  }
}

async fn home_handler(user_data: Option<MySessionData>) -> Html<String> {
  match user_data {
    Some(data) => {
      println!("Retrieved session data: {:?}", data);
      Html(format!("Hello, {}!", data.username))
    }
    None => {
      println!("No session data found.");
      Html("No session data found.".to_string())
    }
  }
}

// Custom extraction of session data from the request parts
#[async_trait]
impl<S> FromRequestParts<S> for MySessionData
where
  S: Send + Sync,
{
  type Rejection = Html<&'static str>;

  async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
    let session = Session::from_request_parts(parts, &state)
      .await
      .map_err(|e| {
        println!("Failed to extract session handle: {}", e.1);
        Html("Failed to extract session handle.")
      })?;
    // Get the session handle from the request extensions
    // let session = parts
    //   .extensions
    //   .get::<Session>()
    //   .ok_or(Html("Session not found"))?;

    // Extract the session data
    if let Ok(Some(data)) = session.get::<MySessionData>("user_data").await {
      Ok(data)
    } else {
      Err(Html("No session data found"))
    }
  }
}

fn generate_random_username() -> String {
  let mut rng = rand::thread_rng();
  let random_suffix: u32 = rng.gen_range(1000 .. 10000); // Generate a random number between 1000 and 9999
  format!("user_{}", random_suffix)
}
