// (module-load "/Users/gerald/personal_infos/rust_example/greet-emacs/target/debug/libgreet_emacs.dylib")

//     (greeting-say-hello "rust")

use emacs::{defun, Env, Result, Value};



emacs::plugin_is_GPL_compatible!();

// 相当于 C 里面的 emacs_module_init
#[emacs::module(name = "greeting")]
fn init(_: &Env) -> Result<()> {
    Ok(())
}

#[defun]
fn say_hello(env: &Env, name: String) -> Result<Value<'_>> {
    env.message(&format!("Hello, {}!", name))
}
