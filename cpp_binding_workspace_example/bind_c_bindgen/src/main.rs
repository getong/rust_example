#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

use std::ffi::CStr;

fn main() {
  unsafe {
    if !SDL_Init(SDL_INIT_VIDEO) {
      eprintln!(
        "SDL_Init failed: {}",
        CStr::from_ptr(SDL_GetError()).to_string_lossy()
      );
      return;
    }

    let window = SDL_CreateWindow(b"Hello World\0".as_ptr() as *const i8, 320, 200, 0);
    if window.is_null() {
      eprintln!(
        "SDL_CreateWindow failed: {}",
        CStr::from_ptr(SDL_GetError()).to_string_lossy()
      );
      SDL_Quit();
      return;
    }

    let renderer = SDL_CreateRenderer(window, std::ptr::null());
    if renderer.is_null() {
      eprintln!(
        "SDL_CreateRenderer failed: {}",
        CStr::from_ptr(SDL_GetError()).to_string_lossy()
      );
      SDL_DestroyWindow(window);
      SDL_Quit();
      return;
    }

    // Bring the window to the front; otherwise it opens behind the
    // currently focused (e.g. fullscreen) app and is easy to miss.
    SDL_RaiseWindow(window);

    // Keep pumping events so the window actually appears and stays
    // responsive; a blocking SDL_Delay would leave it blank on macOS.
    let deadline = SDL_GetTicks() + 2000;
    while SDL_GetTicks() < deadline {
      let mut event: SDL_Event = std::mem::zeroed();
      while SDL_PollEvent(&mut event) {}
      SDL_SetRenderDrawColor(renderer, 40, 90, 160, 255);
      SDL_RenderClear(renderer);
      SDL_RenderPresent(renderer);
      SDL_Delay(10);
    }

    SDL_DestroyRenderer(renderer);
    SDL_DestroyWindow(window);
    SDL_Quit();
  }
}
