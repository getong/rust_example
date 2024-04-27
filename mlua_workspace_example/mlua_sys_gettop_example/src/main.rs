use mlua_sys::lua54::{self, lua_gettop};

fn main() {
  unsafe {
    // Create a new Lua state
    let lua_state = lua54::luaL_newstate();

    // Push some values onto the Lua stack
    lua54::lua_pushinteger(lua_state, 42);
    lua54::lua_pushstring(lua_state, b"hello\0".as_ptr() as *const i8);
    lua54::lua_pushboolean(lua_state, 1);

    // Get the index of the top element on the Lua stack
    let top_index = lua_gettop(lua_state);

    println!("Top index of the Lua stack: {}", top_index);

    // Remember to close the Lua state when done
    lua54::lua_close(lua_state);
  }
}
