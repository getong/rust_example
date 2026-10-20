use rig_core::providers::openai;

#[tokio::main]
async fn main() {
  // Create the OpenAI client and chat model.
  // This requires the `OPENAI_API_KEY` environment variable to be set.
  let openai_client = openai::OpenAI::from_env().expect("OPENAI_API_KEY not found");
  let gpt4 = openai_client.chat("gpt-4");

  // Prompt the model and print its response
  let response = gpt4
    .call("Who are you?")
    .await
    .expect("Failed to prompt GPT-4");

  println!("GPT-4: {}", response.text());
}
