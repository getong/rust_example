#[derive(Debug)]
struct SomeThing<'r> {
  bar: &'r str,
}

fn main() {
  let bar = "bar";
  let foo = SomeThing { bar };
  let debug_display = format!("{:?}", foo);
  let name = debug_display.split("{").nth(0).unwrap();
  println!("{}", name);
}

// copy from https://www.reddit.com/r/rust/comments/x99ojz/simple_macro_to_return_an_instances_struct_name/