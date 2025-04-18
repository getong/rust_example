use std::{collections::BTreeSet, rc::Rc};

use chrono::NaiveDateTime;
use serde::Serialize;
use ts_rs::TS;
use uuid::Uuid;

#[derive(Serialize, TS, Clone)]
#[ts(rename_all = "lowercase")]
#[ts(export, export_to = "UserRole.ts")]
pub enum Role {
  User,
  #[ts(rename = "administrator")]
  Admin,
}

#[derive(Serialize, TS, Clone)]
// when 'serde-compat' is enabled, ts-rs tries to use supported serde attributes.
#[serde(rename_all = "UPPERCASE")]
#[ts(export)]
pub enum Gender {
  Male,
  Female,
  Other,
}

#[derive(Serialize, TS, Clone)]
#[ts(export)]
pub struct User {
  pub user_id: i32,
  pub first_name: String,
  pub last_name: String,
  pub role: Role,
  pub family: Vec<User>,
  #[ts(inline)]
  pub gender: Gender,
  pub token: Uuid,
  #[ts(type = "string")]
  pub created_at: NaiveDateTime,
}

#[derive(Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum Vehicle {
  Bicycle { color: String },
  Car { brand: String, color: String },
}

#[derive(Serialize, TS, Clone)]
#[ts(export)]
pub struct Point<T>
where
  T: TS,
{
  pub time: u64,
  pub value: T,
}

#[derive(Serialize, TS, Clone)]
#[serde(default)]
#[ts(export)]
pub struct Series {
  pub points: Vec<Point<u64>>,
}

#[derive(Serialize, TS)]
#[serde(tag = "kind", content = "d")]
#[ts(export)]
pub enum SimpleEnum {
  A,
  B,
}

#[derive(Serialize, TS)]
#[serde(tag = "kind", content = "data")]
#[ts(export)]
pub enum ComplexEnum {
  A,
  B { foo: String, bar: f64 },
  W(SimpleEnum),
  F { nested: SimpleEnum },
  V(Vec<Series>),
  U(Box<User>),
}

#[derive(Serialize, TS)]
#[serde(tag = "kind")]
#[ts(export)]
pub enum InlineComplexEnum {
  A,
  B { foo: String, bar: f64 },
  W(SimpleEnum),
  F { nested: SimpleEnum },
  V(Vec<Series>),
  U(Box<User>),
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ComplexStruct {
  #[serde(default)]
  pub string_tree: Option<Rc<BTreeSet<String>>>,
}
