use std::ffi::{CStr, CString, c_char};

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

struct Window(*mut SDL_Window);

impl Drop for Window {
  fn drop(&mut self) {
    unsafe {
      SDL_DestroyWindow(self.0);
    }
  }
}

impl Window {
  fn new(title: &str, w: i32, h: i32, flags: u64) -> Result<Window, String> {
    let title = CString::new(title).unwrap();
    let window_ptr = unsafe { SDL_CreateWindow(title.as_ptr(), w, h, flags) };

    if window_ptr.is_null() {
      return Err(sdl_error());
    }

    Ok(Window(window_ptr))
  }

  fn raise(&self) {
    unsafe {
      SDL_RaiseWindow(self.0);
    }
  }
}

struct Renderer(*mut SDL_Renderer);

impl Drop for Renderer {
  fn drop(&mut self) {
    unsafe {
      SDL_DestroyRenderer(self.0);
    }
  }
}

impl Renderer {
  fn new(window: &Window) -> Result<Renderer, String> {
    let renderer_ptr = unsafe { SDL_CreateRenderer(window.0, std::ptr::null()) };

    if renderer_ptr.is_null() {
      return Err(sdl_error());
    }

    Ok(Renderer(renderer_ptr))
  }

  fn present_color(&self, r: u8, g: u8, b: u8) {
    unsafe {
      SDL_SetRenderDrawColor(self.0, r, g, b, 255);
      SDL_RenderClear(self.0);
      SDL_RenderPresent(self.0);
    }
  }
}

fn run() -> Result<(), String> {
  let window = Window::new("Hello World", 320, 200, 0)?;
  let renderer = Renderer::new(&window)?;

  // Bring the window to the front; otherwise it opens behind the
  // currently focused (e.g. fullscreen) app and is easy to miss.
  window.raise();

  // Keep pumping events so the window actually appears and stays
  // responsive; a blocking SDL_Delay would leave it blank on macOS.
  unsafe {
    let deadline = SDL_GetTicks() + 2000;
    while SDL_GetTicks() < deadline {
      let mut event = SDL_Event([0; 128]);
      while SDL_PollEvent(&mut event) {}
      renderer.present_color(40, 90, 160);
      SDL_Delay(10);
    }
  }

  Ok(())
}

fn main() {
  if !unsafe { SDL_Init(SDL_INIT_VIDEO) } {
    eprintln!("SDL_Init failed: {}", sdl_error());
    return;
  }

  if let Err(err) = run() {
    eprintln!("Error: {}", err);
  }

  unsafe {
    SDL_Quit();
  }
}
