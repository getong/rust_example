use std::ffi::{CStr, c_char};

const SDL_INIT_VIDEO: u32 = 0x20;

#[repr(C)]
struct SDL_Window([u8; 0]);

#[repr(C)]
struct SDL_Renderer([u8; 0]);

// SDL_Event is a 128-byte union; we only need its size and alignment
// since we never inspect the events we drain.
#[repr(C, align(8))]
struct SDL_Event([u8; 128]);

#[link(name = "SDL3")]
unsafe extern "C" {
  fn SDL_Init(flags: u32) -> bool;
  fn SDL_Quit();
  fn SDL_GetError() -> *const c_char;
  fn SDL_CreateWindow(title: *const c_char, w: i32, h: i32, flags: u64) -> *mut SDL_Window;
  fn SDL_RaiseWindow(window: *mut SDL_Window);
  fn SDL_DestroyWindow(window: *mut SDL_Window);
  fn SDL_CreateRenderer(window: *mut SDL_Window, name: *const c_char) -> *mut SDL_Renderer;
  fn SDL_SetRenderDrawColor(renderer: *mut SDL_Renderer, r: u8, g: u8, b: u8, a: u8) -> bool;
  fn SDL_RenderClear(renderer: *mut SDL_Renderer) -> bool;
  fn SDL_RenderPresent(renderer: *mut SDL_Renderer) -> bool;
  fn SDL_DestroyRenderer(renderer: *mut SDL_Renderer);
  fn SDL_PollEvent(event: *mut SDL_Event) -> bool;
  fn SDL_GetTicks() -> u64;
  fn SDL_Delay(ms: u32);
}

fn sdl_error() -> String {
  unsafe {
    CStr::from_ptr(SDL_GetError())
      .to_string_lossy()
      .into_owned()
  }
}

fn main() {
  let name = c"Hello World";

  unsafe {
    if !SDL_Init(SDL_INIT_VIDEO) {
      eprintln!("SDL_Init failed: {}", sdl_error());
      return;
    }

    let window = SDL_CreateWindow(name.as_ptr(), 320, 200, 0);
    if window.is_null() {
      eprintln!("SDL_CreateWindow failed: {}", sdl_error());
      SDL_Quit();
      return;
    }

    let renderer = SDL_CreateRenderer(window, std::ptr::null());
    if renderer.is_null() {
      eprintln!("SDL_CreateRenderer failed: {}", sdl_error());
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
      let mut event = SDL_Event([0; 128]);
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
