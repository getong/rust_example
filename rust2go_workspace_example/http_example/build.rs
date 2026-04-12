use rust2go::RegenArgs;

fn main() {
  println!("cargo:rustc-link-lib=framework=CoreFoundation");
  println!("cargo:rustc-link-lib=framework=Security");

  rust2go::Builder::new()
    .with_go_src("./go")
    .with_regen_arg(RegenArgs {
      src: "./src/http.rs".into(),
      dst: "./go/gen.go".into(),
      go118: true,
      ..Default::default()
    })
    .build();
}
