use rig::{completion::Prompt, providers::openai};

#[tokio::main]
async fn main() {
  // Create OpenAI client and agent.
  // This requires the `OPENAI_API_KEY` environment variable to be set.
  let openai_client = openai::Client::from_env();

  let gpt4 = openai_client.agent("gpt-4").build();

  // Prompt the model and print its response
  let response = gpt4
    .prompt("Who are you?")
    .await
    .expect("Failed to prompt GPT-4");

  println!("GPT-4: {response}");
}
