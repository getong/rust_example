use std::time::Duration;

use mlua::{prelude::LuaError, HookTriggers, Lua, Result as LuaResult};

fn set_hook_example() -> LuaResult<()> {
  let lua = Lua::new();
  lua.set_hook(HookTriggers::EVERY_LINE, |_lua, debug| {
    // println!("line {}", debug.curr_line());
    println!("stack number: {}", debug.stack().num_params);
    Ok(())
  });

  _ = lua
    .load(
      r#"
      local x = 2 + 3
      local y = x * 63
      local z = string.len(x..", "..y)
  "#,
    )
    .exec();
  Ok(())
}

fn inpect_stack_example() -> LuaResult<()> {
  let lua = Lua::new();

  let map_table = lua.create_table()?;
  map_table.set(1, "one")?;
  map_table.set("two", 2)?;

  lua.globals().set("map_table", map_table)?;

  let build = lua.load("for k,v in pairs(map_table) do print(k,v) end");

  _ = build.exec();
  match lua.inspect_stack(0) {
    Some(debug) => println!("current stack number: {}", debug.stack().num_params),
    None => println!("the current stack is none"),
  }
  Ok(())
}

async fn inspect_stack_example2() -> LuaResult<()> {
  let lua = Lua::new();

  // Create an asynchronous function for the custom require function
  let require_fn = lua.create_async_function(sleep)?;

  // Create a synchronous function to get the source of the Lua code at a specific stack depth
  let get_source_fn = lua.create_function(move |lua, _: ()| {
    // Inspect the Lua stack at depth 2
    match lua.inspect_stack(0) {
      None => {
        println!("not info found");
        // If stack inspection fails, return a LuaError
        Err(LuaError::runtime(
          "Failed to get stack info for require source",
        ))
      }
      Some(info) => {
        println!("info: {:?}", info.stack().num_params);
        // If stack inspection succeeds, get the source of the Lua code
        match info.source().source {
          None => {
            // If the source is not available, return a LuaError
            Err(LuaError::runtime(
              "Stack info is missing source for require",
            ))
          }
          Some(source) => {
            // If the source is available, create a Lua string from it and return
            lua.create_string(source.as_bytes())
          }
        }
      }
    }
  })?;

  _ = lua.globals().set("sleep", require_fn);
  _ = lua.globals().set("inspect", get_source_fn);
  _ = lua.load("sleep(2);inspect()").call_async(100).await?;
  Ok(())
}

#[tokio::main]
async fn main() -> LuaResult<()> {
  _ = set_hook_example();

  _ = inpect_stack_example();

  _ = inspect_stack_example2().await;
  Ok(())
}

// Assume require is a custom function implemented elsewhere in your Rust code
async fn sleep(_lua: &Lua, n: u64) -> LuaResult<&'static str> {
  tokio::time::sleep(Duration::from_millis(n)).await;
  Ok("done")
}
