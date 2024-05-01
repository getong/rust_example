use boa_engine::{Context, JsResult, Source};

fn main() -> JsResult<()> {
  let js_code = r#"
      let two = 1 + 1;
      let definitely_not_four = two + "2";

      definitely_not_four
  "#;

  // Instantiate the execution context
  let mut context = Context::default();

  // Parse the source code
  let result = context.eval(Source::from_bytes(js_code))?;

  println!("{}", result.display());

  Ok(())
}
