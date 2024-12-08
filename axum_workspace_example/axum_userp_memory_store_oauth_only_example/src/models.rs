use userp::{
  prelude::*,
  reexports::{
    chrono::{DateTime, Utc},
    uuid::Uuid,
  },
};

#[derive(Debug, Clone)]
pub struct MyUser {
  pub id: Uuid,
}

impl User for MyUser {
  fn get_id(&self) -> Uuid {
    self.id
  }
}

#[derive(Debug, Clone)]
pub struct MyLoginSession {
  pub id: Uuid,
  pub user_id: Uuid,
  pub method: LoginMethod,
}

impl LoginSession for MyLoginSession {
  fn get_id(&self) -> Uuid {
    self.id
  }

  fn get_user_id(&self) -> Uuid {
    self.user_id
  }

  fn get_method(&self) -> LoginMethod {
    self.method.clone()
  }
}

#[derive(Clone, Debug)]
#[allow(unused)]
pub struct MyOAuthToken {
  pub id: Uuid,
  pub user_id: Uuid,
  pub provider_name: String,
  pub provider_user_id: String,
  pub access_token: String,
  pub refresh_token: Option<String>,
  pub expires: Option<DateTime<Utc>>,
  pub scopes: Vec<String>,
}

impl OAuthToken for MyOAuthToken {
  fn get_id(&self) -> Uuid {
    self.id
  }

  fn get_user_id(&self) -> Uuid {
    self.user_id
  }

  fn get_provider_name(&self) -> &str {
    self.provider_name.as_str()
  }

  fn get_refresh_token(&self) -> &Option<String> {
    &self.refresh_token
  }
}
