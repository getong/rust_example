use std::path::PathBuf;

fn main() {
  let dst = cmake::Config::new("SDL3-3.4.12")
    .profile("Release")
    .define("SDL_SHARED", "OFF")
    .define("SDL_STATIC", "ON")
    .build();

  println!(
    "cargo:rustc-link-search=native={}",
    dst.join("lib").display()
  );

  if cfg!(target_os = "windows") {
    println!("cargo:rustc-link-lib=static=SDL3-static");
  } else {
    println!("cargo:rustc-link-lib=static=SDL3");
  }

  if cfg!(target_os = "macos") {
    for framework in [
      "Cocoa",
      "IOKit",
      "CoreVideo",
      "CoreAudio",
      "AudioToolbox",
      "ForceFeedback",
      "Carbon",
      "Metal",
      "GameController",
      "CoreHaptics",
      // SDL3 additionally needs these:
      "AVFoundation",           // camera backend
      "CoreMedia",              // camera backend
      "QuartzCore",             // CAMetalLayer
      "UniformTypeIdentifiers", // clipboard UTType
    ] {
      println!("cargo:rustc-link-lib=framework={framework}");
    }
    println!("cargo:rustc-link-lib=iconv");
    println!("cargo:rustc-link-lib=objc");
  }

  let bindings = bindgen::Builder::default()
    .header("wrapper.h")
    // SDL3 headers include each other as <SDL3/...>
    .clang_arg("-ISDL3-3.4.12/include")
    .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
    // bindgen misrepresents the alignment of ARM NEON tuple types
    // (e.g. int8x8x2_t) pulled in via arm_neon.h, so the generated
    // layout tests fail to compile on Apple Silicon.
    .layout_tests(false)
    // Only bind SDL items; without this, bindgen also declares libc
    // functions (malloc, memcpy, ...) with c_ulong instead of usize,
    // which trips the suspicious_runtime_symbol_definitions lint.
    .allowlist_function("SDL_.*")
    .allowlist_type("SDL_.*")
    .allowlist_var("SDL_.*")
    .generate()
    .expect("Unable to generate bindings");

  let out_path = PathBuf::from(std::env::var("OUT_DIR").unwrap());
  bindings
    .write_to_file(out_path.join("bindings.rs"))
    .expect("Couldn't write bindings");
}
