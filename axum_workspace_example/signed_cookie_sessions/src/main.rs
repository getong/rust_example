use axum::{
  Form, Router,
  extract::FromRef,
  http::StatusCode,
  response::{IntoResponse, Redirect},
  routing::{get, post},
};
use axum_extra::extract::cookie::{Cookie, Key, SameSite, SignedCookieJar};
use serde::Deserialize;
use time::Duration;
use tokio::net::TcpListener;

#[derive(Clone)]
struct AppState {
  cookie_key: Key,
}

impl FromRef<AppState> for Key {
  fn from_ref(state: &AppState) -> Self {
    state.cookie_key.clone()
  }
}

#[derive(Deserialize)]
struct Login {
  user_id: String,
}

async fn create_session(jar: SignedCookieJar, Form(login): Form<Login>) -> impl IntoResponse {
  let cookie = Cookie::build(("session_user", login.user_id))
    .path("/")
    .http_only(true)
    .same_site(SameSite::Lax)
    .max_age(Duration::days(7));

  (jar.add(cookie), Redirect::to("/dashboard"))
}

async fn current_user(jar: SignedCookieJar) -> Result<String, StatusCode> {
  match jar.get("session_user") {
    Some(cookie) => Ok(format!("Logged in as {}", cookie.value())),
    None => Err(StatusCode::UNAUTHORIZED),
  }
}

async fn logout(jar: SignedCookieJar) -> impl IntoResponse {
  (jar.remove(Cookie::from("session_user")), Redirect::to("/"))
}

async fn home() -> &'static str {
  "POST user_id to /login, then open /dashboard. POST /logout to clear the session."
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
  let state = AppState {
    cookie_key: Key::generate(),
  };

  let app = Router::new()
    .route("/", get(home))
    .route("/login", post(create_session))
    .route("/dashboard", get(current_user))
    .route("/logout", post(logout))
    .with_state(state);

  let listener = TcpListener::bind("127.0.0.1:3000").await?;
  axum::serve(listener, app).await
}
