use anyhow::Result;
use async_trait::async_trait;
use axum_session_auth::Auth;
use http::Method;
// use redis::{aio::Connection, AsyncCommands, FromRedisValue};
// use redis::AsyncCommands;

use axum_session::SessionStore;
use axum_session_auth::HasPermission;
use axum_session_auth::Rights;

use axum::{routing::get, Router};
use axum_session::{
  // AxumDatabasePool, SessionPgPool, AxumSession, AxumSessionConfig, AxumSessionLayer,
  // SessionPgPool,
  SessionConfig,
  SessionLayer,
};

use axum_session_auth::SessionPgPool;
use axum_session_auth::{AuthConfig, AuthSession, AuthSessionLayer, Authentication};
use sqlx::PgPool;

use axum_macros::debug_handler;

#[tokio::main]
async fn main() {
  //# async {
  let poll = connect_to_database().await.unwrap();

  let session_config = SessionConfig::default().with_table_name("test_table");
  let auth_config = AuthConfig::<i64>::default().with_anonymous_user_id(Some(1));
  let session_store =
    SessionStore::<SessionPgPool>::new(Some(poll.clone().into()), session_config).await;

  // Build our application with some routes
  let router = Router::new()
    .route("/greet/:name", get(greet))
    .layer(SessionLayer::new(
      session_store.expect("session store not initialized"),
    ))
    .layer(
      AuthSessionLayer::<User, i64, SessionPgPool, PgPool>::new(Some(poll))
        .with_config(auth_config),
    );

  // Run it
  let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
  tracing::debug!("listening on {:?}", listener);
  axum::serve(listener, router).await.unwrap();
}

// We can get the Method to compare with what Methods we allow. Useful if this supports multiple methods.
// When called auth is loaded in the background for you.
#[debug_handler]
async fn greet(method: Method, auth: AuthSession<User, i64, SessionPgPool, PgPool>) -> String {
  let mut count: usize = auth.session.get("count").unwrap_or(0);
  count += 1;

  // Session is Also included with Auth so no need to require it in the function arguments if your using
  // AuthSession.
  _ = auth.session.set("count", count);

  // If for some reason you needed to update your Users Permissions or data then you will want to clear the user cache if it is enabled.
  // The user Cache is enabled by default. To clear simply use.
  auth.cache_clear_user(1);
  //or to clear all for a large update
  auth.cache_clear_all();

  if let Some(ref cur_user) = auth.current_user {
    if !Auth::<User, i64, PgPool>::build([Method::GET], false)
      .requires(Rights::none([
        Rights::permission("Token::UseAdmin"),
        Rights::permission("Token::ModifyPerms"),
      ]))
      .validate(&cur_user, &method, None)
      .await
    {
      return format!("No Permissions! for {}", cur_user.username);
    }

    let username = if !auth.is_authenticated() {
      // Set the user ID of the User to the Session so it can be Auto Loaded the next load or redirect
      _ = auth.login_user(2);
      "".to_string()
    } else {
      // If the user is loaded and is Authenticated then we can use it.
      if let Some(user) = auth.current_user {
        user.username.clone()
      } else {
        "".to_string()
      }
    };

    format!("{}-{}", username, count)
  } else {
    if !auth.is_authenticated() {
      // Set the user ID of the User to the Session so it can be Auto Loaded the next load or redirect
      _ = auth.login_user(2);
      // Set the session to be long term. Good for Remember me type instances.
      _ = auth.remember_user(true);
      // Redirect here after login if we did indeed login.
    }

    "No Permissions!".to_owned()
  }
}

#[derive(Clone, Debug)]
pub struct User {
  pub id: i64,
  pub anonymous: bool,
  pub username: String,
}

// This is only used if you want to use Token based Authentication checks
#[async_trait]
impl HasPermission<PgPool> for User {
  async fn has(&self, perm: &str, _pool: &Option<&PgPool>) -> bool {
    match &perm[..] {
      "Token::UseAdmin" => true,
      "Token::ModifyUser" => true,
      _ => false,
    }
  }
}

#[async_trait]
impl Authentication<User, i64, PgPool> for User {
  async fn load_user(userid: i64, _pool: Option<&PgPool>) -> Result<User> {
    Ok(User {
      id: userid,
      anonymous: true,
      username: "Guest".to_string(),
    })
  }

  fn is_authenticated(&self) -> bool {
    !self.anonymous
  }

  fn is_active(&self) -> bool {
    !self.anonymous
  }

  fn is_anonymous(&self) -> bool {
    self.anonymous
  }
}

async fn connect_to_database() -> anyhow::Result<sqlx::Pool<sqlx::Postgres>> {
  Ok(sqlx::PgPool::connect("DATABASE_URL").await.unwrap())
}
