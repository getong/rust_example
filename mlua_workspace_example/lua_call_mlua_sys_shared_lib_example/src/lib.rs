use std::ffi::CStr;

use mlua_sys::lua54::{
  lua_Integer, lua_State, lua_gettop, lua_pushcfunction, lua_pushinteger, lua_settop, lua_type,
  lua_typename,
};

extern "C-unwind" fn _c_l_testfunc(lua_state: *mut lua_State) -> i32 {
  unsafe {
    let argc = lua_gettop(lua_state) as usize;

    if argc != 0 {
      println!("共传入 {} 个参数", argc);
      for index in 1 ..= argc {
        let type_name_ptr = lua_typename(lua_state, lua_type(lua_state, index as i32));
        let type_name_cstr = CStr::from_ptr(type_name_ptr);
        let type_name_str = type_name_cstr.to_str().unwrap();
        println!("第 {} 个参数类型为: {}", index, type_name_str);
      }
    } else {
      println!("0 个参数传入");
    }

    // 清空栈
    lua_settop(lua_state, 0);

    // 把参数个数压入栈作为返回值
    lua_pushinteger(lua_state, argc as lua_Integer);
  }

  // Return the number of return values
  1
}

/// # Safety
///
/// This function should be called by lua script
#[no_mangle]
pub unsafe extern "C" fn luaopen_funct(lua_state: *mut lua_State) -> i32 {
  unsafe {
    lua_pushcfunction(lua_state, _c_l_testfunc);
    1
  }
}
