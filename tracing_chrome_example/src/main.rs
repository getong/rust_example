use tracing_chrome::ChromeLayerBuilder;
use tracing_subscriber::{prelude::*, registry::Registry};

#[tokio::main]
async fn main() {
    let (chrome_layer, _guard) = ChromeLayerBuilder::new().build();
    tracing_subscriber::registry().with(chrome_layer).init();
    println!("Hello, world!");
}
