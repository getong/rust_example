use windows::Win32::UI::WindowsAndMessaging::{MessageBoxA, MB_OK};

fn main() {
  unsafe {
    MessageBoxA(None, "Hello Windows 11 from Rust", "HelloMsg", MB_OK);
  }
}
