use recoco::{builder::FlowBuilder, execution::evaluator::evaluate_transient_flow, prelude::*};
use serde_json::json;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  recoco::lib_context::init_lib_context(Some(recoco::settings::Settings::default())).await?;

  let mut builder = FlowBuilder::new("hello_world").await?;

  let input = builder.add_direct_input(
    "text".to_string(),
    schema::make_output_type(schema::BasicValueType::Str),
  )?;

  let output = builder
    .transform(
      "SplitBySeparators".to_string(),
      json!({
        "separators_regex": [" "],
        "keep_separator": null,
        "include_empty": false,
        "trim": true
      })
        .as_object()
        .unwrap()
        .clone(),
      vec![(input, Some("text".to_string()))],
      None,
      "splitter".to_string(),
    )
    .await?;

  builder.set_direct_output(output)?;

  let flow = builder.build_transient_flow().await?;
  let result = evaluate_transient_flow(
    &flow.0,
    &vec![value::Value::Basic(value::BasicValue::Str(
      "Hello Recoco".into(),
    ))],
  )
  .await?;

  println!("Result: {:?}", result);
  Ok(())
}
