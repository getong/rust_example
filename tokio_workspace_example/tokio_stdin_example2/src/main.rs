use tokio::io::{AsyncBufReadExt, BufReader};

#[tokio::main]
async fn main() {
  let stdin = tokio::io::stdin();
  let mut reader = BufReader::new(stdin);

  println!("Type 'quit' to exit.");

  loop {
    let mut line = String::new();

    match reader.read_line(&mut line).await {
      Ok(0) => break, // End of input
      Ok(_) => {
        let input = line.trim();
        if input == "quit" {
          break;
        }

        println!("Input: {}", input);
      }
      Err(err) => {
        eprintln!("Failed to read input: {}", err);
        break;
      }
    }
  }

  println!("Goodbye!");
}
