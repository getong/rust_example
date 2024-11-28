use std::fs;

use mlua::{Lua, Result};

fn main() -> Result<()> {
  let lua = Lua::new(); // Create a new Lua state
  let script_content = fs::read_to_string("src/factorial.lua")?; // Read the Lua script file

  // Load and execute the Lua script
  lua.load(&script_content).exec()?;

  Ok(())
}
