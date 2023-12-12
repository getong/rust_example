use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(Hello)]
pub fn hello(item: TokenStream) -> TokenStream {
  let ast = parse_macro_input!(item as DeriveInput);
  let name = ast.ident;
  let add_hello_world = quote! {
      impl #name {
          fn hello_world(&self) {
              println!("Hello world")
          }
      }
  };
  add_hello_world.into()
}
