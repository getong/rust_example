use mlua::{Lua, Result, Value};

fn main() -> Result<()> {
  let lua = Lua::new(); // Create a new Lua state

  // Define a Lua variable
  lua.load(r#"x = 42; y = "hello"; z = {1, 2, 3}"#).exec()?;

  // Retrieve and check the type of variable `x`
  let x: Value = lua.globals().get("x")?;
  match x {
    Value::Number(_) => println!("x is a number"),
    Value::String(_) => println!("x is a string"),
    Value::Integer(_) => println!("x is an integer"),
    _ => println!("x is other type"),
  }

  // Retrieve and check the type of variable `y`
  let y: Value = lua.globals().get("y")?;
  match y {
    Value::Number(_) => println!("y is a number"),
    Value::String(_) => println!("y is a string"),
    _ => println!("y is another type"),
  }

  // Retrieve and check the type of variable `z`
  let z: Value = lua.globals().get("z")?;
  match z {
    Value::Table(_) => println!("z is a table"),
    _ => println!("z is another type"),
  }

  Ok(())
}
