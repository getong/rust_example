use rustyscript::{Module, Runtime};
use uuid::Uuid;

fn main() -> Result<(), Box<dyn std::error::Error>> {
  // Load .env file
  dotenvy::dotenv().ok();

  // Get environment variables
  let api_key = std::env::var("STREAM_API_KEY")?;
  let api_secret = std::env::var("STREAM_API_SECRET")?;

  // Generate a UUID for user_id
  let user_id = Uuid::new_v4().to_string();

  // Create runtime with Node.js experimental support
  let runtime_options = rustyscript::RuntimeOptions {
      extensions: vec![],
      ..Default::default()
  };
  let mut runtime = Runtime::new(runtime_options)?;

  // Load the TypeScript module
  let module = Module::load("metrics_client/dist/create-token.js")?;
  let handle = runtime.load_module(&module)?;

  // Call the createUserToken function (which returns a Promise)
  let token: String = runtime.call_function(
      Some(&handle),
    "createUserToken",
    rustyscript::json_args!(api_key, api_secret, user_id.clone()),
  )?;

  println!("Generated token for user {}: {}", user_id, token);

  Ok(())
}
