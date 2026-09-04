use libretranslate::{Language, TranslateError, translate_url};

#[tokio::main]
async fn main() -> Result<(), TranslateError> {
  let source = Language::English;
  let target = Language::Chinese;

  let input = "hello world";

  let data = translate_url(source, target, input, "http://localhost:5050/", None).await?;

  println!("Input {}: {}", data.source.as_pretty(), data.input);
  println!("Output {}: {}", data.target.as_pretty(), data.output);

  Ok(())
}
