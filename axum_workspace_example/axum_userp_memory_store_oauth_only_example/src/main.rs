mod models;
mod store;
mod templates;

use askama_axum::IntoResponse;
use axum::{Router, extract::State, response::Redirect, routing::get, serve};
use axum_macros::FromRef;
use dotenv::var;
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use userp::prelude::{url::Url, *};

use self::{
  store::MemoryStore,
  templates::{IndexTemplate, ProtectedTemplate},
};

#[derive(Clone, FromRef)]
struct AppState {
  store: MemoryStore,
  auth: UserpConfig,
}

#[tokio::main]
async fn main() {
  tracing_subscriber::fmt()
    .with_max_level(tracing::Level::DEBUG)
    .init();

  let req_var =
    |name: &'static str| var(name).unwrap_or_else(|_| panic!("Missing required env var: {name}"));

  let base_url = Url::parse("http://localhost:3000").unwrap();

  let key = String::from(
    "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
  );

  let auth = UserpConfig::new(
    key,
    Routes::default(),
    OAuthConfig::new(base_url)
      .with_client(SpotifyOAuthProvider::new(
        req_var("SPOTIFY_CLIENT_ID"),
        req_var("SPOTIFY_CLIENT_SECRET"),
      ))
      .with_client(GitHubOAuthProvider::new(
        req_var("GITHUB_CLIENT_ID"),
        req_var("GITHUB_CLIENT_SECRET"),
      )),
  )
  .with_allow_signup(Allow::OnSelf)
  .with_allow_login(Allow::OnSelf)
  .with_https_only(false);

  let auth_router = auth.router::<MemoryStore, AppState>();

  let state = AppState {
    store: MemoryStore::default(),
    auth,
  };

  let app = Router::new()
    .merge(auth_router)
    .route("/store", get(get_store))
    .route("/", get(get_index))
    .route("/protected", get(get_protected))
    .layer(TraceLayer::new_for_http())
    .with_state(state);

  println!("Userp example listening at http://localhost:3000 :)");

  let tcp = TcpListener::bind("0.0.0.0:3000").await.unwrap();

  serve(tcp, app.into_make_service()).await.unwrap();
}

async fn get_index(auth: Userp<MemoryStore>) -> impl IntoResponse {
  let logged_in = auth.logged_in().await.unwrap();

  IndexTemplate { logged_in }
}

async fn get_store(State(state): State<AppState>) -> impl IntoResponse {
  format!("{:#?}", state.store).into_response()
}

async fn get_protected(auth: Userp<MemoryStore>) -> impl IntoResponse {
  let Some((user, session)) = auth.user_session().await.unwrap() else {
    return Redirect::to(&format!(
      "/login?next={}",
      urlencoding::encode("/protected")
    ))
    .into_response();
  };

  ProtectedTemplate {
    user: format!("{user:#?}"),
    session: format!("{session:#?}"),
  }
  .into_response()
}
