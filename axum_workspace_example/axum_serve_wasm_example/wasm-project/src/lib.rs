mod utils;

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn plus1(x: i32) -> i32 {
    x+1
}

#[wasm_bindgen]
extern "C" {
    fn alert(s: &str);
}

#[wasm_bindgen]
pub fn greet() {
    alert("Hello, wasm-project!");
}
