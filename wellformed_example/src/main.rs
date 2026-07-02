use serde_json::json;
use wellformed::{
  Constraint, ErrorMeta, Predicate, Schema, Transform, TypeSchema,
  ir::{ObjectSchema, StringSchema},
  validate,
};

fn main() -> wellformed::Result<()> {
  let schema = Schema::new(
    "1.0",
    TypeSchema::Object(ObjectSchema::new().property(
      "email",
      TypeSchema::String(StringSchema::new().transform(Transform::trim()).constraint(
        Constraint::new(
          Predicate::call("is_email", serde_json::Value::Null),
          ErrorMeta::new("INVALID_EMAIL", "Enter a valid email address"),
        ),
      )),
    )),
  );

  let mut value = json!({ "email": " ada@example.com " });
  let result = validate(&schema, &mut value)?;

  assert!(result.is_valid());
  assert_eq!(value["email"], "ada@example.com");
  Ok(())
}
