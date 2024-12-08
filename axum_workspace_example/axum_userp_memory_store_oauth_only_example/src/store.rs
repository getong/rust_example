use std::{collections::HashMap, sync::Arc};

use axum::{
  async_trait,
  http::StatusCode,
  response::{IntoResponse, Response},
};
use tokio::sync::RwLock;
use userp::{
  prelude::*,
  reexports::{thiserror, uuid::Uuid},
};

use crate::models::{MyLoginSession, MyOAuthToken, MyUser};

#[derive(Clone, Default, Debug)]
pub struct MemoryStore {
  sessions: Arc<RwLock<HashMap<Uuid, MyLoginSession>>>,
  users: Arc<RwLock<HashMap<Uuid, MyUser>>>,
  oauth_tokens: Arc<RwLock<HashMap<Uuid, MyOAuthToken>>>,
}

#[derive(thiserror::Error, Debug)]
pub enum MemoryStoreError {
  #[error("The token was not found: {0}")]
  TokenNotFound(String),
  #[error("The user ID did not match")]
  UserMissmatch,
}

impl IntoResponse for MemoryStoreError {
  fn into_response(self) -> Response {
    (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()).into_response()
  }
}

#[async_trait]
impl UserpStore for MemoryStore {
  type User = MyUser;
  type LoginSession = MyLoginSession;
  type OAuthToken = MyOAuthToken;
  type Error = MemoryStoreError;

  async fn get_session(&self, session_id: Uuid) -> Result<Option<Self::LoginSession>, Self::Error> {
    let sessions = self.sessions.read().await;

    Ok(sessions.get(&session_id).cloned())
  }

  async fn delete_session(&self, user_id: Uuid, session_id: Uuid) -> Result<(), Self::Error> {
    let mut sessions = self.sessions.write().await;

    let session = sessions.remove(&session_id);

    match session {
      Some(session) if session.user_id != user_id => Err(MemoryStoreError::UserMissmatch),
      _ => Ok(()),
    }
  }

  async fn create_session(
    &self,
    user_id: Uuid,
    method: LoginMethod,
  ) -> Result<Self::LoginSession, Self::Error> {
    let session = MyLoginSession {
      id: Uuid::new_v4(),
      user_id,
      method,
    };

    let mut sessions = self.sessions.write().await;

    sessions.insert(session.id, session.clone());

    Ok(session)
  }

  async fn get_user(&self, user_id: Uuid) -> Result<Option<MyUser>, Self::Error> {
    let users = self.users.read().await;

    Ok(users.get(&user_id).cloned())
  }

  async fn update_token_by_unmatched_token(
    &self,
    token_id: Uuid,
    unmatched_token: UnmatchedOAuthToken,
  ) -> Result<Self::OAuthToken, Self::Error> {
    let mut tokens = self.oauth_tokens.write().await;

    let prev = tokens
      .get_mut(&token_id)
      .ok_or(MemoryStoreError::TokenNotFound(token_id.to_string()))?;

    prev.provider_name = unmatched_token.provider_name;
    prev.provider_user_id = unmatched_token.provider_user_id;
    prev.access_token = unmatched_token.access_token;
    prev.refresh_token = unmatched_token.refresh_token;
    prev.expires = unmatched_token.expires;
    prev.scopes = unmatched_token.scopes;

    Ok(prev.clone())
  }

  async fn oauth_get_token_by_id(
    &self,
    token_id: Uuid,
  ) -> Result<Option<Self::OAuthToken>, Self::Error> {
    let tokens = self.oauth_tokens.read().await;

    Ok(tokens.get(&token_id).cloned())
  }

  async fn get_token_by_unmatched_token(
    &self,
    unmatched_token: UnmatchedOAuthToken,
  ) -> Result<Option<Self::OAuthToken>, Self::Error> {
    let tokens = self.oauth_tokens.read().await;

    Ok(
      tokens
        .values()
        .find(|t| {
          t.provider_name == unmatched_token.provider_name
            && t.provider_user_id == unmatched_token.provider_user_id
        })
        .cloned(),
    )
  }

  async fn create_user_token_from_unmatched_token(
    &self,
    user_id: Uuid,
    unmatched_token: UnmatchedOAuthToken,
  ) -> Result<Self::OAuthToken, Self::Error> {
    let mut tokens = self.oauth_tokens.write().await;

    let token = Self::OAuthToken {
      id: Uuid::new_v4(),
      user_id,
      provider_name: unmatched_token.provider_name,
      provider_user_id: unmatched_token.provider_user_id,
      access_token: unmatched_token.access_token,
      refresh_token: unmatched_token.refresh_token,
      expires: unmatched_token.expires,
      scopes: unmatched_token.scopes,
    };

    tokens.insert(token.id, token.clone());

    Ok(token)
  }

  async fn create_user_from_unmatched_token(
    &self,
    unmatched_token: UnmatchedOAuthToken,
  ) -> Result<(Self::User, Self::OAuthToken), Self::Error> {
    let mut tokens = self.oauth_tokens.write().await;
    let mut users = self.users.write().await;

    let user = Self::User { id: Uuid::new_v4() };

    let token = Self::OAuthToken {
      id: Uuid::new_v4(),
      user_id: user.id,
      provider_name: unmatched_token.provider_name,
      provider_user_id: unmatched_token.provider_user_id,
      access_token: unmatched_token.access_token,
      refresh_token: unmatched_token.refresh_token,
      expires: unmatched_token.expires,
      scopes: unmatched_token.scopes,
    };

    tokens.insert(token.id, token.clone());
    users.insert(user.id, user.clone());

    Ok((user, token))
  }

  async fn get_user_by_unmatched_token(
    &self,
    unmatched_token: UnmatchedOAuthToken,
  ) -> Result<Option<(Self::User, Self::OAuthToken)>, Self::Error> {
    let tokens = self.oauth_tokens.read().await;
    let users = self.users.read().await;

    Ok(
      tokens
        .values()
        .find(|t| {
          t.provider_name == unmatched_token.provider_name
            && t.provider_user_id == unmatched_token.provider_user_id
        })
        .and_then(|t| users.get(&t.user_id).map(|u| (u.clone(), t.clone()))),
    )
  }
}
