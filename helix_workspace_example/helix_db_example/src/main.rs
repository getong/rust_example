use helix_db_example::{
  create_user_query, delete_user_query, read_user_query, remote_client, run_dynamic_query,
  update_user_query,
};

const HELIX_DB_URL: &str = "http://127.0.0.1:6969";
const DEMO_EXTERNAL_ID: &str = "rust-sdk-crud-demo";

#[tokio::main]
async fn main() -> Result<(), helix_db::HelixError> {
  let client = remote_client(HELIX_DB_URL, "")?;

  let cleanup: sonic_rs::Value =
    run_dynamic_query(&client, delete_user_query(DEMO_EXTERNAL_ID)).await?;
  println!("cleanup: {cleanup}");

  let created: sonic_rs::Value = run_dynamic_query(
    &client,
    create_user_query(DEMO_EXTERNAL_ID, "Alice", "active"),
  )
  .await?;
  println!("create: {created}");

  let read_after_create: sonic_rs::Value =
    run_dynamic_query(&client, read_user_query(DEMO_EXTERNAL_ID)).await?;
  println!("read after create: {read_after_create}");

  let updated: sonic_rs::Value = run_dynamic_query(
    &client,
    update_user_query(DEMO_EXTERNAL_ID, "Alice Updated", "inactive"),
  )
  .await?;
  println!("update: {updated}");

  let read_after_update: sonic_rs::Value =
    run_dynamic_query(&client, read_user_query(DEMO_EXTERNAL_ID)).await?;
  println!("read after update: {read_after_update}");

  let deleted: sonic_rs::Value =
    run_dynamic_query(&client, delete_user_query(DEMO_EXTERNAL_ID)).await?;
  println!("delete: {deleted}");

  let read_after_delete: sonic_rs::Value =
    run_dynamic_query(&client, read_user_query(DEMO_EXTERNAL_ID)).await?;
  println!("read after delete: {read_after_delete}");

  Ok(())
}
