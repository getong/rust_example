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
}
