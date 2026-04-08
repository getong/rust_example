use std::env;

use dotenvy::dotenv;
use supabase_client_sdk::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  dotenv()?;

  let supabase_url = env::var("SUPABASE_URL").expect("SUPABASE_URL must be set in the .env file");
  let supabase_anon_key =
    env::var("SUPABASE_ANON_KEY").expect("SUPABASE_ANON_KEY must be set in the .env file");

  let config = SupabaseConfig::new(&supabase_url, &supabase_anon_key);

  let client = SupabaseClient::new(config)?;

  let response = client.from("cities").select("*").execute().await;

  for row in response.into_result()? {
    println!("{}", row.get_as::<String>("name").unwrap());
  }

  Ok(())
}
