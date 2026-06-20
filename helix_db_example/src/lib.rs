use helix_db::{Client, dsl::prelude::*};
use serde::{Deserialize, Serialize};

pub fn user_and_friends_query() -> ReadBatch {
  read_batch()
    .var_as(
      "user",
      g().n_where(SourcePredicate::eq("username", "alice")),
    )
    .var_as(
      "friends",
      g()
        .n(NodeRef::var("user"))
        .out(Some("FOLLOWS"))
        .dedup()
        .limit(100),
    )
    .returning(["user", "friends"])
}

pub fn active_users_query() -> ReadBatch {
  read_batch()
    .var_as(
      "active_users",
      g()
        .n_with_label_where("User", SourcePredicate::eq("status", "active"))
        .where_(Predicate::gt("score", 100i64))
        .order_by("score", Order::Desc)
        .limit(25)
        .value_map(Some(vec!["$id", "name", "score"])),
    )
    .returning(["active_users"])
}

pub fn matching_users_query() -> ReadBatch {
  let statuses = Expr::param("statuses");

  read_batch()
    .var_as(
      "matching_users",
      g()
        .n_with_label("User")
        .where_(Predicate::is_in_expr("status", statuses))
        .value_map(Some(vec!["$id", "name", "status"])),
    )
    .returning(["matching_users"])
}

pub fn user_posts_if_found_query() -> ReadBatch {
  read_batch()
    .var_as(
      "user",
      g().n_where(SourcePredicate::eq("username", "alice")),
    )
    .var_as_if(
      "posts",
      BatchCondition::VarNotEmpty("user".to_string()),
      g().n(NodeRef::var("user")).out(Some("POSTED")),
    )
    .returning(["user", "posts"])
}

pub fn create_alice_bob_follow_query() -> WriteBatch {
  write_batch()
    .var_as(
      "alice",
      g().add_n("User", vec![("name", "Alice"), ("tier", "pro")]),
    )
    .var_as("bob", g().add_n("User", vec![("name", "Bob")]))
    .var_as(
      "linked",
      g()
        .n(NodeRef::var("alice"))
        .add_e(
          "FOLLOWS",
          NodeRef::var("bob"),
          vec![("since", "2026-01-01")],
        )
        .count(),
    )
    .returning(["alice", "bob", "linked"])
}

pub fn deactivate_inactive_users_query() -> WriteBatch {
  write_batch()
    .var_as(
      "inactive_users",
      g().n_with_label_where("User", SourcePredicate::eq("status", "inactive")),
    )
    .var_as_if(
      "deactivated_count",
      BatchCondition::VarNotEmpty("inactive_users".to_string()),
      g()
        .n(NodeRef::var("inactive_users"))
        .set_property("deactivated", true)
        .count(),
    )
    .returning(["deactivated_count"])
}

pub fn local_client() -> Result<Client, helix_db::HelixError> {
  Client::new(None)
}

pub fn remote_client(url: &str, api_key: &str) -> Result<Client, helix_db::HelixError> {
  Ok(Client::new(Some(url))?.with_api_key(Some(api_key)))
}

#[register]
pub fn add_user(name: String) -> WriteBatch {
  write_batch()
    .var_as("user_id", g().add_n("user", vec![("name", name)]))
    .returning(["user_id"])
}

#[derive(Debug, Deserialize)]
pub struct AddUserResponse {
  pub user_id: u64,
}

#[derive(Debug, Deserialize)]
pub struct MyResponse {}

#[derive(Debug, Serialize)]
pub struct StoredAddUserPayload {
  pub name: String,
}

pub async fn run_dynamic_add_user(
  client: &Client,
  name: String,
) -> Result<AddUserResponse, helix_db::HelixError> {
  let request = add_user(name);
  client.query().dynamic(request).send().await
}

pub async fn run_dynamic_query<R>(
  client: &Client,
  request: DynamicQueryRequest,
) -> Result<R, helix_db::HelixError>
where
  R: for<'de> Deserialize<'de>,
{
  client.query().dynamic(request).send().await
}

pub async fn run_stored_add_user(
  client: &Client,
  payload: &StoredAddUserPayload,
) -> Result<AddUserResponse, helix_db::HelixError> {
  client
    .query()
    .body(payload)?
    .stored("add_user".to_string())
    .send()
    .await
}

pub fn sample_batches() -> (Vec<ReadBatch>, Vec<WriteBatch>) {
  (
    vec![
      user_and_friends_query(),
      active_users_query(),
      matching_users_query(),
      user_posts_if_found_query(),
    ],
    vec![
      create_alice_bob_follow_query(),
      deactivate_inactive_users_query(),
    ],
  )
}
