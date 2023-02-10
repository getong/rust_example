use tokio::io::{AsyncBufReadExt, BufReader};

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Result<T> = std::result::Result<T, Error>;

#[tokio::main]
async fn main() -> Result<()> {
    // println!("Hello, world!");

    let stdin = tokio::io::stdin();
    let mut lines = BufReader::new(stdin).lines();

    while let Some(line) = lines.next_line().await? {
        println!("length = {}", line.len());
    }
    Ok(())
}
