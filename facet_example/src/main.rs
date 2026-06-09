use facet::{Facet, Shape, Type, UserType};

#[derive(Debug, Facet)]
struct Config {
  name: String,
  port: u16,
  #[facet(sensitive)]
  api_key: String,
}

#[derive(Debug, Facet)]
#[repr(u8)]
#[facet(rename_all = "snake_case")]
enum Environment {
  Local,
  Staging,
  Production,
}

#[derive(Debug, Facet)]
struct Service {
  #[facet(rename = "service_name")]
  name: String,
  #[facet(default)]
  replicas: u16,
  environment: Environment,
}

fn main() {
  let config = Config {
    name: String::from("demo-api"),
    port: 8080,
    api_key: String::from("sk_live_123456"),
  };

  println!("Config value: {config:?}");
  print_shape::<Config>("Config struct");
  print_shape::<Service>("Service struct with attributes");
  print_shape::<Vec<String>>("Vec<String> built-in implementation");
  print_shape::<Option<u16>>("Option<u16> built-in implementation");
  print_shape::<Environment>("Environment enum");
}

fn print_shape<'facet, T>(title: &str)
where
  T: Facet<'facet>,
{
  println!("\n== {title} ==");
  describe_shape(T::SHAPE);
}

fn describe_shape(shape: &'static Shape) {
  println!("type identifier: {}", shape.type_identifier);
  println!("effective name: {}", shape.effective_name());
  println!("type: {}", shape.ty);
  println!("definition: {:?}", shape.def);

  match &shape.ty {
    Type::User(UserType::Struct(struct_type)) => {
      println!("fields:");
      for field in struct_type.fields {
        let name = field.rename.unwrap_or(field.name);
        let default = if field.has_default() { " default" } else { "" };
        let sensitive = if field.is_sensitive() {
          " sensitive"
        } else {
          ""
        };
        println!(
          "  - {name}: {}{default}{sensitive}",
          field.shape.get().type_identifier
        );
      }
    }
    Type::User(UserType::Enum(enum_type)) => {
      println!("variants:");
      for variant in enum_type.variants {
        println!("  - {}", variant.effective_name());
      }
    }
    _ => {}
  }
}
