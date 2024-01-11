// #[lang = "fn"]
// pub trait Fn<Args>: FnMut<Args> {
//     extern "rust-call" fn call(&self, args: Args) -> Self::Output;
// }

fn fn_immut<F>(func: F)
where
  F: Fn(),
{
  func();
  func();
}

#[derive(Debug)]
pub struct E {
  pub a: String,
}

fn main() {
  // println!("Hello, world!");
  let e = E {
    a: "fn".to_string(),
  };

  let f = || {
    println!("Fn closure calls: {:?}", e);
  };

  fn_immut(f);
}
