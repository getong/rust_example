// use mongodb::bson::{self, doc, Bson};
//use std::env;
use std::error::Error;

use mongodb::{options::ClientOptions, Client};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Load the MongoDB connection string from an environment variable:
    // let client_uri = env::var("MONGODB_URI").expect("mongodb://localhost:27017");
    // A Client is needed to connect to MongoDB:
    // An extra line of code to work around a DNS issue on Windows:

    // Parse a connection string into an options struct.
    let mut client_options =
        ClientOptions::parse("mongodb://mongoadmin:secret@localhost:27010").await?;

    // Manually set an option.
    client_options.app_name = Some("My App".to_string());

    let client = Client::with_options(client_options)?;
    // Print the databases in our MongoDB cluster:
    println!("Databases:");
    for name in client.list_database_names(None, None).await? {
        println!("- {}", name);
    }
    Ok(())
}
