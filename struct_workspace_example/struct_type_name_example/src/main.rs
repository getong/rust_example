struct SomeThing;

fn _get_name<T>(_: &T) -> &'static str {
  std::any::type_name::<T>()
}

macro_rules! name_struct {
  ($e:expr) => {
    _get_name(&$e)
  };
}

fn main() {
  let thing = SomeThing {};
  let name = name_struct!(thing);
  // current package is struct_type_name_example
  // so print struct_type_name_example::SomeThing
  // on playground prints: "playground::SomeThing"
  println!("{name}");
}

// copy from https://www.reddit.com/r/rust/comments/x99ojz/simple_macro_to_return_an_instances_struct_name/
