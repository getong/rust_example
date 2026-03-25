use std::{
  env,
  time::{SystemTime, UNIX_EPOCH},
};

use dotenv::dotenv;
use serde_json::{Map, Value, json};
use supabase_rs::SupabaseClient;

type AppResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Debug, Clone)]
struct AppConfig {
  schema: String,
  table: String,
  run_write_examples: bool,
  run_rpc_examples: bool,
  rpc_function: String,
  rpc_params_json: Option<String>,
  filter_column: Option<String>,
  filter_value: Option<String>,
  in_filter_column: Option<String>,
  in_filter_values_csv: Option<String>,
  text_search_column: Option<String>,
  text_search_value: Option<String>,
  demo_name_column: String,
  demo_status_column: String,
}

impl AppConfig {
  fn from_env() -> Self {
    Self {
      schema: read_env_or("SUPABASE_SCHEMA", "public"),
      table: read_env_or("SUPABASE_EXAMPLE_TABLE", "todos"),
      run_write_examples: read_env_bool("SUPABASE_RUN_WRITE_EXAMPLES", false),
      run_rpc_examples: read_env_bool("SUPABASE_RUN_RPC_EXAMPLES", false),
      rpc_function: read_env_or("SUPABASE_RPC_FUNCTION", "health_check"),
      rpc_params_json: read_env_optional("SUPABASE_RPC_PARAMS_JSON"),
      filter_column: read_env_optional("SUPABASE_FILTER_COLUMN"),
      filter_value: read_env_optional("SUPABASE_FILTER_VALUE"),
      in_filter_column: read_env_optional("SUPABASE_IN_FILTER_COLUMN"),
      in_filter_values_csv: read_env_optional("SUPABASE_IN_FILTER_VALUES"),
      text_search_column: read_env_optional("SUPABASE_TEXT_SEARCH_COLUMN"),
      text_search_value: read_env_optional("SUPABASE_TEXT_SEARCH_VALUE"),
      demo_name_column: read_env_or("SUPABASE_DEMO_NAME_COLUMN", "name"),
      demo_status_column: read_env_or("SUPABASE_DEMO_STATUS_COLUMN", "status"),
    }
  }
}

#[tokio::main]
async fn main() -> AppResult<()> {
  // Load environment variables from .env file
  dotenv().ok();

  let config = AppConfig::from_env();

  // Initialize the Supabase client.
  let client = SupabaseClient::new(env::var("SUPABASE_URL")?, env::var("SUPABASE_KEY")?)?
    .schema(&config.schema);

  println!(
    "[OK] Supabase client initialized. schema={}, table={}",
    config.schema, config.table
  );

  run_read_examples(&client, &config).await;

  if config.run_write_examples {
    run_write_examples(&client, &config).await;
  } else {
    println!(
      "[INFO] Skip write examples. Set SUPABASE_RUN_WRITE_EXAMPLES=true to enable \
       insert/update/delete demos."
    );
  }

  if config.run_rpc_examples {
    run_rpc_examples(&client, &config).await;
  } else {
    println!("[INFO] Skip RPC examples. Set SUPABASE_RUN_RPC_EXAMPLES=true to enable rpc demos.");
  }

  Ok(())
}

async fn run_read_examples(client: &SupabaseClient, config: &AppConfig) {
  print_section("Read Examples");

  match client.from(&config.table).limit(5).execute().await {
    Ok(rows) => {
      println!("select + limit: got {} rows", rows.len());
      if let Some(first) = rows.first() {
        println!("first row preview: {}", first);
      }
    }
    Err(err) => {
      println!("select + limit failed: {}", err);
    }
  }

  match client.from(&config.table).range(0, 2).execute().await {
    Ok(rows) => println!("range(0,2): got {} rows", rows.len()),
    Err(err) => println!("range example failed: {}", err),
  }

  match client
    .from(&config.table)
    .columns(vec!["id"])
    .limit(1)
    .first()
    .await
  {
    Ok(Some(row)) => {
      println!("columns + first: {}", row);

      if let Some(id_filter) = row.get("id").and_then(value_to_filter_string) {
        match client
          .from(&config.table)
          .eq("id", &id_filter)
          .single()
          .await
        {
          Ok(single_row) => println!("eq + single (by id): {}", single_row),
          Err(err) => println!("eq + single failed: {}", err),
        }
      } else {
        println!("eq + single skipped: `id` field not found in first row");
      }
    }
    Ok(None) => {
      println!("columns + first: table has no rows");
    }
    Err(err) => {
      println!("columns + first failed: {}", err);
    }
  }

  if let (Some(column), Some(value)) = (&config.filter_column, &config.filter_value) {
    match client
      .from(&config.table)
      .eq(column, value)
      .count()
      .limit(10)
      .execute()
      .await
    {
      Ok(rows) => println!(
        "eq + count on {}={}: got {} rows",
        column,
        value,
        rows.len()
      ),
      Err(err) => println!("eq + count failed: {}", err),
    }
  } else {
    println!("eq + count skipped: set SUPABASE_FILTER_COLUMN and SUPABASE_FILTER_VALUE");
  }

  if let (Some(column), Some(values_csv)) = (&config.in_filter_column, &config.in_filter_values_csv)
  {
    let values: Vec<String> = values_csv
      .split(',')
      .map(str::trim)
      .filter(|item| !item.is_empty())
      .map(str::to_owned)
      .collect();

    if values.is_empty() {
      println!("in_ skipped: SUPABASE_IN_FILTER_VALUES has no valid values");
    } else {
      let value_refs: Vec<&str> = values.iter().map(String::as_str).collect();
      match client
        .from(&config.table)
        .in_(column, &value_refs)
        .limit(10)
        .execute()
        .await
      {
        Ok(rows) => println!("in_ filter on {}: got {} rows", column, rows.len()),
        Err(err) => println!("in_ filter failed: {}", err),
      }
    }
  } else {
    println!("in_ skipped: set SUPABASE_IN_FILTER_COLUMN and SUPABASE_IN_FILTER_VALUES");
  }

  if let (Some(column), Some(value)) = (&config.text_search_column, &config.text_search_value) {
    match client
      .from(&config.table)
      .text_search(column, value)
      .limit(10)
      .execute()
      .await
    {
      Ok(rows) => println!("text_search on {}: got {} rows", column, rows.len()),
      Err(err) => println!("text_search failed: {}", err),
    }
  } else {
    println!("text_search skipped: set SUPABASE_TEXT_SEARCH_COLUMN and SUPABASE_TEXT_SEARCH_VALUE");
  }
}

async fn run_write_examples(client: &SupabaseClient, config: &AppConfig) {
  print_section("Write Examples");

  let now = now_unix_seconds();
  let created_name = format!("supabase-rs-demo-{}", now);
  let mut insert_payload = Map::new();
  insert_payload.insert(
    config.demo_name_column.clone(),
    Value::String(created_name.clone()),
  );
  insert_payload.insert(
    config.demo_status_column.clone(),
    Value::String("created".to_owned()),
  );

  let inserted_id_raw = match client
    .insert_without_defined_key(&config.table, Value::Object(insert_payload))
    .await
  {
    Ok(id) => {
      println!("insert_without_defined_key succeeded, raw id={}", id);
      id
    }
    Err(err) => {
      println!("insert_without_defined_key failed: {}", err);
      println!(
        "write examples stopped. Please adjust SUPABASE_EXAMPLE_TABLE/SUPABASE_DEMO_* columns to \
         your schema."
      );
      return;
    }
  };

  let inserted_id = normalize_inserted_id(&inserted_id_raw);

  let mut update_payload = Map::new();
  update_payload.insert(
    config.demo_status_column.clone(),
    Value::String("updated".to_owned()),
  );

  match client
    .update(&config.table, &inserted_id, Value::Object(update_payload))
    .await
  {
    Ok(id) => println!("update succeeded, id={}", id),
    Err(err) => println!("update failed: {}", err),
  }

  let mut upsert_payload = Map::new();
  upsert_payload.insert(
    config.demo_name_column.clone(),
    Value::String(format!("{}-upsert", created_name)),
  );
  upsert_payload.insert(
    config.demo_status_column.clone(),
    Value::String("upserted".to_owned()),
  );

  match client
    .upsert(&config.table, &inserted_id, Value::Object(upsert_payload))
    .await
  {
    Ok(id) => println!("upsert succeeded, id={}", id),
    Err(err) => println!("upsert failed: {}", err),
  }

  match client.delete(&config.table, &inserted_id).await {
    Ok(_) => println!("delete succeeded, cleaned id={}", inserted_id),
    Err(err) => println!("delete failed: {}", err),
  }

  let alt_name = format!("supabase-rs-demo-alt-{}", now + 1);
  let mut alt_payload = Map::new();
  alt_payload.insert(
    config.demo_name_column.clone(),
    Value::String(alt_name.clone()),
  );
  alt_payload.insert(
    config.demo_status_column.clone(),
    Value::String("temporary".to_owned()),
  );

  match client
    .insert_without_defined_key(&config.table, Value::Object(alt_payload))
    .await
  {
    Ok(_) => match client
      .delete_without_defined_key(&config.table, &config.demo_name_column, &alt_name)
      .await
    {
      Ok(_) => println!(
        "delete_without_defined_key succeeded by {}={}",
        config.demo_name_column, alt_name
      ),
      Err(err) => println!("delete_without_defined_key failed: {}", err),
    },
    Err(err) => println!(
      "secondary insert for delete_without_defined_key failed: {}",
      err
    ),
  }
}

async fn run_rpc_examples(client: &SupabaseClient, config: &AppConfig) {
  print_section("RPC Examples");

  let params = match &config.rpc_params_json {
    Some(raw) => match serde_json::from_str::<Value>(raw) {
      Ok(value) => value,
      Err(err) => {
        println!(
          "invalid SUPABASE_RPC_PARAMS_JSON, fallback to {{}}. parse error: {}",
          err
        );
        json!({})
      }
    },
    None => json!({}),
  };

  match client
    .rpc(&config.rpc_function, params.clone())
    .limit(10)
    .execute()
    .await
  {
    Ok(rows) => println!("rpc execute succeeded, rows={}", rows.len()),
    Err(err) => println!("rpc execute failed: {}", err),
  }

  match client
    .rpc(&config.rpc_function, params)
    .execute_single()
    .await
  {
    Ok(value) => println!("rpc execute_single succeeded: {}", value),
    Err(err) => println!("rpc execute_single failed: {}", err),
  }
}

fn read_env_or(name: &str, default_value: &str) -> String {
  match env::var(name) {
    Ok(value) if !value.trim().is_empty() => value,
    _ => default_value.to_owned(),
  }
}

fn read_env_optional(name: &str) -> Option<String> {
  match env::var(name) {
    Ok(value) if !value.trim().is_empty() => Some(value),
    _ => None,
  }
}

fn read_env_bool(name: &str, default_value: bool) -> bool {
  match env::var(name) {
    Ok(value) => {
      let normalized = value.trim().to_ascii_lowercase();
      normalized == "1" || normalized == "true" || normalized == "yes" || normalized == "on"
    }
    Err(_) => default_value,
  }
}

fn value_to_filter_string(value: &Value) -> Option<String> {
  match value {
    Value::Null => None,
    Value::String(inner) => Some(inner.clone()),
    Value::Bool(inner) => Some(inner.to_string()),
    Value::Number(inner) => Some(inner.to_string()),
    Value::Array(_) | Value::Object(_) => Some(value.to_string()),
  }
}

fn normalize_inserted_id(raw: &str) -> String {
  if let Ok(parsed) = serde_json::from_str::<Value>(raw) {
    if let Some(normalized) = value_to_filter_string(&parsed) {
      return normalized;
    }
  }

  raw.trim_matches('"').to_owned()
}

fn now_unix_seconds() -> u64 {
  match SystemTime::now().duration_since(UNIX_EPOCH) {
    Ok(duration) => duration.as_secs(),
    Err(_) => 0,
  }
}

fn print_section(title: &str) {
  println!("\n===== {} =====", title);
}
