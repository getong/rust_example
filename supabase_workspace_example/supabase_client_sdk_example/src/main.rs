use std::{env, io};

use dotenvy::dotenv;
use serde_json::json;
use supabase_client_sdk::{prelude::*, supabase_client_auth::AdminCreateUserParams};

type DynError = Box<dyn std::error::Error>;

const CITY_NAMES: &[&str] = &[
  "Tokyo",
  "Osaka",
  "Shanghai",
  "Singapore",
  "Seoul",
  "Shenzhen",
  "San Francisco",
  "New York",
];

const DEMO_USERS: &[(&str, &str, &str)] = &[
  ("sdk-demo-alice@example.com", "SupabaseDemo123!", "Alice"),
  ("sdk-demo-bob@example.com", "SupabaseDemo123!", "Bob"),
];

fn required_env(keys: &[&str]) -> Result<String, DynError> {
  for key in keys {
    if let Ok(value) = env::var(key) {
      if !value.trim().is_empty() {
        return Ok(value);
      }
    }
  }

  Err(
    io::Error::new(
      io::ErrorKind::InvalidInput,
      format!(
        "missing required environment variable, tried {}",
        keys.join(", ")
      ),
    )
    .into(),
  )
}

fn build_client(url: &str, api_key: &str) -> Result<SupabaseClient, DynError> {
  let config = SupabaseConfig::new(url, api_key);
  Ok(SupabaseClient::new(config)?)
}

fn json_scalar_to_string(value: &serde_json::Value) -> String {
  match value {
    serde_json::Value::Null => "null".to_string(),
    serde_json::Value::Bool(v) => v.to_string(),
    serde_json::Value::Number(v) => v.to_string(),
    serde_json::Value::String(v) => v.clone(),
    other => other.to_string(),
  }
}

async fn seed_cities(client: &SupabaseClient) -> Result<(), DynError> {
  println!("1. Writing more data into Supabase");

  for city_name in CITY_NAMES {
    let response = client
      .from("cities")
      .upsert(row![("name", *city_name)])
      .on_conflict(&["name"])
      .ignore_duplicates()
      .select()
      .execute()
      .await;

    let rows = response.into_result()?;
    if rows.is_empty() {
      println!("  kept existing city: {city_name}");
    } else {
      println!("  inserted city: {city_name}");
    }
  }

  Ok(())
}

async fn recreate_demo_auth_users(service_client: &SupabaseClient) -> Result<(), DynError> {
  println!("\n2. Making Supabase auth data");

  let service_auth = service_client.auth()?;
  let admin = service_auth.admin();
  let existing_users = admin.list_users(None, None).await?;

  for (email, password, display_name) in DEMO_USERS {
    for existing in existing_users
      .users
      .iter()
      .filter(|user| user.email.as_deref() == Some(*email))
    {
      admin.delete_user(&existing.id).await?;
      println!("  deleted existing auth user: {email}");
    }

    let created_user = admin
      .create_user(AdminCreateUserParams {
        email: Some((*email).to_string()),
        password: Some((*password).to_string()),
        email_confirm: Some(true),
        user_metadata: Some(json!({
          "display_name": display_name,
          "seeded_by": "supabase_client_sdk_example",
        })),
        app_metadata: Some(json!({
          "role": "demo_user",
          "source": "rust-example",
        })),
        ..Default::default()
      })
      .await?;

    println!("  created auth user: {} ({})", email, created_user.id,);
  }

  Ok(())
}

async fn verify_auth_sign_in(anon_client: &SupabaseClient) -> Result<(), DynError> {
  let auth = anon_client.auth()?;
  let (email, password, _) = DEMO_USERS[0];

  let session = auth.sign_in_with_password_email(email, password).await?;
  let current_user = auth.get_user(&session.access_token).await?;
  let claims = AuthClient::get_claims(&session.access_token)?;

  println!("\n3. Generating JWT token");
  println!("  jwt token: {}", session.access_token);
  println!(
    "  signed in email: {}",
    current_user.email.as_deref().unwrap_or("<none>")
  );
  println!("  signed in user id: {}", current_user.id);
  println!("  jwt role claim: {}", claims["role"]);
  println!("  jwt subject claim: {}", claims["sub"]);

  auth.sign_out_current().await?;

  Ok(())
}

async fn read_table_data(client: &SupabaseClient) -> Result<(), DynError> {
  println!("\n4. Reading Supabase table data");

  let response = client
    .from("cities")
    .select("id, name")
    .order("id", OrderDirection::Ascending)
    .execute()
    .await;

  for row in response.into_result()? {
    println!(
      "  city row: id={} name={}",
      row.get_as::<i64>("id").unwrap_or_default(),
      row.get_as::<String>("name").unwrap_or_default(),
    );
  }

  Ok(())
}

async fn read_auth_data(service_client: &SupabaseClient) -> Result<(), DynError> {
  let service_auth = service_client.auth()?;
  let admin = service_auth.admin();
  let users = admin.list_users(None, None).await?;

  println!("\n5. Reading Supabase auth data");

  for user in users
    .users
    .iter()
    .filter(|user| match user.email.as_deref() {
      Some(email) => DEMO_USERS
        .iter()
        .any(|(demo_email, _, _)| demo_email == &email),
      None => false,
    })
  {
    println!(
      "  auth user: id={} email={} confirmed={}",
      user.id,
      user.email.as_deref().unwrap_or("<none>"),
      user.email_confirmed_at.is_some(),
    );
  }

  Ok(())
}

async fn search_data_with_graphql(
  _service_client: &SupabaseClient,
  anon_client: &SupabaseClient,
) -> Result<(), DynError> {
  let auth = anon_client.auth()?;
  let (email, password, _) = DEMO_USERS[0];
  let session = auth.sign_in_with_password_email(email, password).await?;

  let graphql = anon_client.graphql()?;
  graphql.set_auth(&session.access_token);

  println!("\n6. Using GraphQL to search data");

  let connection = graphql
    .collection("citiesCollection")
    .select(&["id", "name"])
    .filter(GqlFilter::ilike("name", "%sh%"))
    .order_by("name", OrderByDirection::AscNullsLast)
    .first(10)
    .execute::<serde_json::Value>()
    .await?;

  println!("  graphql matched rows: {}", connection.edges.len());

  for edge in connection.edges {
    println!(
      "  graphql city: id={} name={}",
      json_scalar_to_string(&edge.node["id"]),
      edge.node["name"].as_str().unwrap_or_default(),
    );
  }

  auth.sign_out_current().await?;

  Ok(())
}

#[tokio::main]
async fn main() -> Result<(), DynError> {
  dotenv()?;

  let supabase_url = required_env(&["SUPABASE_URL"])?;
  let anon_key = required_env(&["SUPABASE_ANON_KEY", "SUPABASE_API_KEY"])?;
  let service_role_key = required_env(&["SUPABASE_SERVICE_ROLE_KEY", "SUPABASE_KEY"])?;

  let anon_client = build_client(&supabase_url, &anon_key)?;
  let service_client = build_client(&supabase_url, &service_role_key)?;

  seed_cities(&service_client).await?;
  recreate_demo_auth_users(&service_client).await?;
  verify_auth_sign_in(&anon_client).await?;
  read_table_data(&service_client).await?;
  read_auth_data(&service_client).await?;
  search_data_with_graphql(&service_client, &anon_client).await?;

  Ok(())
}
