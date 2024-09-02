#[tokio::main]
async fn main() {
  println!("Hello, world!");
}

// use lsp-bridge-rust-expand-macro to generate
// fn main() {
//   let body = async {
//     {
//       $crate::io::_print($crate::format_args_nl!("Hello, world!"));
//     };
//   };
//   #[allow(clippy::expect_used, clippy::diverging_sub_expression)]
//   {
//     return tokio::runtime::Builder::new_multi_thread()
//       .enable_all()
//       .build()
//       .expect("Failed building the Runtime")
//       .block_on(body);
//   }
// }
