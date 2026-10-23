fn main() {
  if cfg!(target_os = "macos") {
    // Homebrew installs GStreamer and its GLib dependencies outside the
    // default macOS dynamic-library search paths.
    println!("cargo:rustc-link-arg=-Wl,-rpath,/opt/homebrew/lib");
    println!("cargo:rustc-link-arg=-Wl,-rpath,/opt/homebrew/opt/glib/lib");
    println!("cargo:rustc-link-arg=-Wl,-rpath,/opt/homebrew/opt/gstreamer/lib");
  }
}
