use anyhow::*;
use std::path::Path;
use tokio::fs;

#[tokio::main]
async fn main() -> Result<()> {
    // println!("Hello, world!");
    let config_local_path = Path::new("Cargo.toml");

    let doc = if config_local_path.exists() {
        let toml = fs::read_to_string(&config_local_path).await?;
        toml.parse::<toml_edit::Document>()?
    } else {
        toml_edit::Document::new()
    };

    println!("doc: {:?}", doc);
    Ok(())
}
